//! The table: one row a changed definition, entry points first and then by file
//! and line, members right after their class, then one row a file for what no
//! definition claims. A definition is listed when a changed line falls inside
//! it, and the text before its body compared across the sides says whether its
//! interface changed. Its TSV form is seam's oldest interface, so `to_tsv` and
//! `from_tsv` keep its twenty columns exactly.
use crate::diff::{Hunks, Side};
use crate::lang::Languages;
use crate::{FileChange, Status};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub path: String,
    pub kind: String,
    pub name: String,
    pub class: String,
    pub exported: Option<bool>,
    pub change: String,
    pub side: String,
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub open: Option<u32>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub head: String,
    pub base_start: Option<u32>,
    pub base_end: Option<u32>,
    pub staged: Option<bool>,
    pub unstaged: Option<bool>,
    pub uses: Vec<String>,
    pub root: Option<bool>,
    pub new_uses: Vec<String>,
}

impl Row {
    pub fn is_file(&self) -> bool {
        self.kind == "file"
    }
    /// `path#name`, or `path#Class.name` for a member.
    pub fn id(&self) -> String {
        if self.class.is_empty() {
            format!("{}#{}", self.path, self.name)
        } else {
            format!("{}#{}.{}", self.path, self.class, self.name)
        }
    }
}

fn num<T: ToString>(v: &Option<T>) -> String {
    v.as_ref().map_or(String::new(), T::to_string)
}
fn flag(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "1",
        Some(false) => "0",
        None => "",
    }
}

pub fn to_tsv(rows: &[Row]) -> String {
    let mut out = String::new();
    for r in rows {
        let cells: [&str; 20] = [
            &r.path,
            &r.kind,
            &r.name,
            &r.class,
            flag(r.exported),
            &r.change,
            &r.side,
            &num(&r.start),
            &num(&r.end),
            &num(&r.open),
            &num(&r.added),
            &num(&r.removed),
            &r.head,
            &num(&r.base_start),
            &num(&r.base_end),
            flag(r.staged),
            flag(r.unstaged),
            &r.uses.join("|"),
            flag(r.root),
            &r.new_uses.join("|"),
        ];
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}

/// Any table with these columns, whoever wrote it: a cell missing or not a
/// number reads as empty.
pub fn from_tsv(text: &str) -> Vec<Row> {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let c: Vec<&str> = line.split('\t').collect();
            let s = |i: usize| c.get(i).copied().unwrap_or("").to_string();
            let n32 = |i: usize| c.get(i).and_then(|v| v.parse().ok());
            let n64 = |i: usize| c.get(i).and_then(|v| v.parse().ok());
            let b = |i: usize| match c.get(i).copied().unwrap_or("") {
                "" => None,
                v => Some(v == "1"),
            };
            let list = |i: usize| match c.get(i).copied().unwrap_or("") {
                "" => vec![],
                v => v.split('|').map(str::to_string).collect(),
            };
            Row {
                path: s(0),
                kind: s(1),
                name: s(2),
                class: s(3),
                exported: b(4),
                change: s(5),
                side: s(6),
                start: n32(7),
                end: n32(8),
                open: n32(9),
                added: n64(10),
                removed: n64(11),
                head: s(12),
                base_start: n32(13),
                base_end: n32(14),
                staged: b(15),
                unstaged: b(16),
                uses: list(17),
                root: b(18),
                new_uses: list(19),
            }
        })
        .collect()
}

/// A declaration on one side of one file.
struct Decl {
    side: Side,
    file: usize,
    rule: String,
    /// The first line, the comment block above it included.
    start: u32,
    /// The line its keyword is on: where enter opens and where export sits.
    keyword: u32,
    end: u32,
    name: String,
    /// The name its users mention: a type sharing a name with a const is `type X`.
    bare: String,
    head: String,
    code: String,
    parent: String,
}

/// The names one definition's text mentions, how many times, and in what order.
///
/// What counts: not after a dot, since `runner.run()` is a property of runner and
/// not the `run` declared here, but after a spread, since `...opts` is a use; and
/// not before a colon or a question mark, where the name is a key or a typed
/// property. Every other character is a boundary. A colon that opens the next
/// line is the other arm of a ternary, not a key, so a newline clears the name.
#[derive(Default)]
struct Mentions<'t> {
    count: HashMap<&'t str, i64>,
    first: HashMap<&'t str, usize>,
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn mentions(text: &str) -> Mentions<'_> {
    let mut m = Mentions::default();
    let bytes = text.as_bytes();
    let mut seen = 0usize;
    let mut prev: Option<&str> = None;
    let mut i = 0;
    while i < bytes.len() {
        // A chunk runs between whitespace; inside one, anything not part of a
        // name, a dot, a colon or a question mark is a hard boundary.
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == 0x0b) {
            if bytes[i] == b'\n' {
                prev = None;
            }
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let chunk_start = i;
        while i < bytes.len() && !(bytes[i].is_ascii_whitespace() || bytes[i] == 0x0b) {
            if bytes[i] == b'.' && text[i..].starts_with("...") {
                // A spread is a boundary of whitespace's kind.
                break;
            }
            i += 1;
        }
        if i == chunk_start {
            i += 3;
            continue;
        }
        let chunk = &text[chunk_start..i];
        // A chunk opening with : or ? unmakes the name before it, which is then
        // a key. Only whitespace may lie between: in `Opts): string` the bracket
        // ends the name and the colon is the return type.
        if let (Some(b':' | b'?'), Some(p)) = (chunk.as_bytes().first(), prev) {
            let c = m.count.entry(p).or_insert(0);
            *c -= 1;
            if *c <= 0 {
                m.count.remove(p);
                m.first.remove(p);
            }
        }
        prev = None;
        let pieces: Vec<&str> = chunk
            .split(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '.' | ':' | '?'))
            })
            .collect();
        let last_piece = pieces.len() - 1;
        for (h, piece) in pieces.iter().enumerate() {
            // Only what follows the last colon or question mark can be a use.
            let seg = piece.rsplit([':', '?']).next().unwrap_or("");
            let Some(first) = seg.as_bytes().first() else {
                continue;
            };
            if !is_name_byte(*first) {
                continue;
            }
            let end = seg
                .bytes()
                .position(|b| !is_name_byte(b))
                .unwrap_or(seg.len());
            let token = &seg[..end];
            *m.count.entry(token).or_insert(0) += 1;
            seen += 1;
            m.first.entry(token).or_insert(seen);
            if h == last_piece {
                prev = Some(token);
            }
        }
    }
    m
}

type LineKey = (Side, usize, u32);

/// Scan both sides of every file, on as many threads as there are.
fn scan_all(
    files: &[FileChange],
    languages: &Languages,
    max: usize,
) -> Result<Vec<Vec<(Side, crate::lang::Match)>>, String> {
    let scanner = languages.scanner()?;
    let one = |f: &FileChange| {
        let mut out = vec![];
        for (side, bytes) in [(Side::Old, &f.old), (Side::New, &f.new)] {
            if let Some(b) = bytes.as_deref().filter(|b| b.len() <= max) {
                out.extend(scanner.scan(&f.path, b).into_iter().map(|m| (side, m)));
            }
        }
        out
    };
    #[cfg(target_arch = "wasm32")]
    {
        Ok(files.iter().map(one).collect())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let next = std::sync::atomic::AtomicUsize::new(0);
        let mut slots: Vec<Vec<(Side, crate::lang::Match)>> = Vec::new();
        slots.resize_with(files.len(), Vec::new);
        let done = std::sync::Mutex::new(slots);
        std::thread::scope(|s| {
            for _ in 0..threads.min(files.len().max(1)) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(f) = files.get(i) else { break };
                    let scanned = one(f);
                    done.lock().expect("no thread panics holding this")[i] = scanned;
                });
            }
        });
        Ok(done.into_inner().expect("no thread panicked"))
    }
}

/// The table of a change. `kinds` says what each path is (tests, gen, cfg, or
/// nothing), since each kind and each language keeps to itself. `max` is the
/// size past which a side is listed as a plain file.
pub fn build(
    files: &[FileChange],
    languages: &Languages,
    kinds: &HashMap<String, String>,
    max: usize,
) -> Result<Vec<Row>, String> {
    let scanned = scan_all(files, languages, max)?;
    let readable: Vec<usize> = (0..files.len())
        .filter(|&i| {
            let under = |b: &Option<Vec<u8>>| b.as_ref().is_some_and(|b| b.len() <= max);
            under(&files[i].old) || under(&files[i].new)
        })
        .collect();
    let empty: Vec<u8> = vec![];
    let mut hunks: Vec<Hunks> = (0..files.len()).map(|_| Hunks::default()).collect();
    for &i in &readable {
        let f = &files[i];
        hunks[i] = Hunks::between(
            f.old.as_ref().unwrap_or(&empty),
            f.new.as_ref().unwrap_or(&empty),
        );
    }

    let mut decls: Vec<Decl> = vec![];
    let mut exported: HashSet<LineKey> = HashSet::new();
    let mut comment_end: HashMap<LineKey, u32> = HashMap::new();
    for (file, matches) in scanned.into_iter().enumerate() {
        for (side, m) in matches {
            match m.rule.as_str() {
                "export" => {
                    exported.insert((side, file, m.start));
                }
                // Comments, by the line each ends on; two on one line keep the first.
                "comment" => {
                    let e = comment_end.entry((side, file, m.end)).or_insert(m.start);
                    *e = (*e).min(m.start);
                }
                _ => decls.push(Decl {
                    side,
                    file,
                    rule: m.rule,
                    start: m.start,
                    keyword: m.start,
                    end: m.end,
                    bare: m.name.clone(),
                    name: m.name,
                    head: m.head,
                    code: m.code,
                    parent: String::new(),
                }),
            }
        }
    }

    // A member belongs to the definition around it on its side. Only the
    // declarations of its own file and side can hold it.
    let mut in_file: HashMap<(Side, usize), Vec<usize>> = HashMap::new();
    for (i, d) in decls.iter().enumerate() {
        in_file.entry((d.side, d.file)).or_default().push(i);
    }
    let is_member = |rule: &str| rule == "method" || rule == "field";
    for i in 0..decls.len() {
        if !is_member(&decls[i].rule) {
            continue;
        }
        let holder = in_file[&(decls[i].side, decls[i].file)]
            .iter()
            .copied()
            .find(|&j| {
                !is_member(&decls[j].rule)
                    && !decls[j].rule.is_empty()
                    && decls[j].start <= decls[i].start
                    && decls[i].end <= decls[j].end
            });
        match holder {
            Some(j) => decls[i].parent = decls[j].name.clone(),
            None => decls[i].rule.clear(),
        }
    }

    // The comment block right above a declaration belongs to it. It runs up while
    // each comment ends on the line before, so a blank line stops it, and so does
    // a comment that trails the declaration before on its last line.
    let ends: HashSet<LineKey> = decls
        .iter()
        .filter(|d| !d.rule.is_empty())
        .map(|d| (d.side, d.file, d.end))
        .collect();
    for d in decls.iter_mut().filter(|d| !d.rule.is_empty()) {
        while let Some(&block) = comment_end.get(&(d.side, d.file, d.start.wrapping_sub(1))) {
            if ends.contains(&(d.side, d.file, block)) {
                break;
            }
            d.start = block;
        }
    }

    // TypeScript keeps types and values apart, so a type may share its name with
    // a const. The type is then named "type X" on both sides, so the two are two
    // symbols; `bare` keeps the name its users mention.
    let values: HashSet<(usize, String)> = decls
        .iter()
        .filter(|d| !d.rule.is_empty() && d.rule != "type" && d.rule != "interface")
        .map(|d| (d.file, d.name.clone()))
        .collect();
    for d in decls.iter_mut() {
        if (d.rule == "type" || d.rule == "interface") && values.contains(&(d.file, d.name.clone()))
        {
            d.name = format!("type {}", d.name);
        }
    }

    let key = |d: &Decl| (d.file, d.parent.clone(), d.name.clone());
    type Key = (usize, String, String);
    let mut old_at: HashMap<Key, usize> = HashMap::new();
    let mut new_at: HashMap<Key, usize> = HashMap::new();
    let mut order_new: Vec<Key> = vec![];
    let mut order_old: Vec<Key> = vec![];
    for (i, d) in decls.iter().enumerate().filter(|(_, d)| !d.rule.is_empty()) {
        let (map, order) = if d.side == Side::Old {
            (&mut old_at, &mut order_old)
        } else {
            (&mut new_at, &mut order_new)
        };
        if map.insert(key(d), i).is_none() {
            order.push(key(d));
        }
    }

    // One row a symbol touched on either side, keyed by file, class and name. A
    // member that changed touches its class too, so the class is always here.
    struct Listed {
        decl: usize,
        change: &'static str,
        added: u64,
        removed: u64,
        open: u32,
    }
    let mut rows: Vec<Listed> = vec![];
    for k in &order_new {
        let i = new_at[k];
        let d = &decls[i];
        let h = &hunks[d.file];
        let touched_new = h.touched(Side::New, d.start, d.end);
        let added = h.lines(Side::New, d.start, d.end);
        if let Some(&o) = old_at.get(k) {
            let od = &decls[o];
            let touched_old = h.touched(Side::Old, od.start, od.end);
            if !touched_new && !touched_old {
                continue;
            }
            let change = if d.head != od.head {
                "signature"
            } else {
                "body"
            };
            rows.push(Listed {
                decl: i,
                change,
                added,
                removed: h.lines(Side::Old, od.start, od.end),
                open: d.keyword,
            });
        } else if touched_new {
            rows.push(Listed {
                decl: i,
                change: "added",
                added,
                removed: 0,
                open: d.keyword,
            });
        }
    }
    for k in &order_old {
        if new_at.contains_key(k) {
            continue;
        }
        let i = old_at[k];
        let d = &decls[i];
        let h = &hunks[d.file];
        if !h.touched(Side::Old, d.start, d.end) {
            continue;
        }
        let open = h.at(d.keyword).unwrap_or(1);
        rows.push(Listed {
            decl: i,
            change: "removed",
            added: 0,
            removed: h.lines(Side::Old, d.start, d.end),
            open,
        });
    }

    let m = rows.len();
    let top = |r: usize| decls[rows[r].decl].parent.is_empty();
    let mut row_of_top: HashMap<(usize, String), usize> = HashMap::new();
    for r in 0..m {
        if top(r) {
            let d = &decls[rows[r].decl];
            row_of_top.insert((d.file, d.name.clone()), r);
        }
    }
    let exported_row = |r: usize| {
        let d = &decls[rows[r].decl];
        exported.contains(&(d.side, d.file, d.keyword))
    };
    // A member is as exported as its class, and says nothing when its class has no row.
    let exp: Vec<Option<bool>> = (0..m)
        .map(|r| {
            if top(r) {
                Some(exported_row(r))
            } else {
                let d = &decls[rows[r].decl];
                row_of_top
                    .get(&(d.file, d.parent.clone()))
                    .map(|&t| exported_row(t))
            }
        })
        .collect();

    // A const and a type of one name where one mentions the other read as one
    // thing: users of the name go to the outer one, and the inner sits under it.
    let mut pair: HashMap<(usize, &str, bool), usize> = HashMap::new();
    for r in (0..m).filter(|&r| top(r)) {
        let d = &decls[rows[r].decl];
        pair.insert((d.file, d.bare.as_str(), d.name == d.bare), r);
    }
    let mut inner: HashMap<usize, usize> = HashMap::new();
    for r in (0..m).filter(|&r| top(r)) {
        let d = &decls[rows[r].decl];
        if d.name == d.bare {
            continue;
        }
        let Some(&q) = pair.get(&(d.file, d.bare.as_str(), true)) else {
            continue;
        };
        // A declaration names itself once, so the two name each other only on a
        // second mention.
        if mentions(&d.code)
            .count
            .get(d.bare.as_str())
            .copied()
            .unwrap_or(0)
            >= 2
        {
            inner.insert(q, r);
            continue;
        }
        let other = &decls[rows[q].decl];
        if mentions(&other.code)
            .count
            .get(d.bare.as_str())
            .copied()
            .unwrap_or(0)
            >= 2
        {
            inner.insert(r, q);
        }
    }

    // The changed definitions indexed by the name they are mentioned as, so what
    // one uses is a lookup rather than a pass against every other.
    let group = |file: usize| -> (&str, &str) {
        let p = files[file].path.as_str();
        (
            kinds.get(p).map_or("", String::as_str),
            languages.name_of(p),
        )
    };
    let mut by_name: HashMap<(&str, &str, &str), Vec<usize>> = HashMap::new();
    for r in (0..m).filter(|&r| top(r)) {
        let d = &decls[rows[r].decl];
        if !d.bare.is_empty() && d.bare.bytes().all(is_name_byte) {
            let (kind, lang) = group(d.file);
            by_name
                .entry((kind, lang, d.bare.as_str()))
                .or_default()
                .push(r);
        }
    }

    let sort_key = |r: usize| {
        let d = &decls[rows[r].decl];
        let side = if d.side == Side::Old { "old" } else { "new" };
        format!("{}{:08}{}", files[d.file].path, d.start, side)
    };
    let mut uses: Vec<Vec<usize>> = vec![vec![]; m];
    let mut new_uses: Vec<Vec<usize>> = vec![vec![]; m];
    let mut indegree = vec![0usize; m];
    for r in (0..m).filter(|&r| top(r)) {
        let d = &decls[rows[r].decl];
        let (kind, lang) = group(d.file);
        let said = mentions(&d.code);
        // In the order they are mentioned; two definitions of one name are named
        // in the same place and take the order they are listed in.
        let mut found: Vec<(usize, usize)> = vec![];
        for (name, count) in &said.count {
            let Some(targets) = by_name.get(&(kind, lang, name)) else {
                continue;
            };
            for &q in targets {
                if q == r || inner.get(&q).is_some_and(|&i| i != r) {
                    continue;
                }
                // A definition names itself once, in its own declaration, so the
                // const and the type of one name use each other only on a second.
                if *name == d.bare && *count < 2 {
                    continue;
                }
                found.push((said.first[name], q));
                indegree[q] += 1;
            }
        }
        found.sort_unstable();
        uses[r] = found.iter().map(|&(_, q)| q).collect();
        // An edge is new when the base side of the user did not name it.
        let old = old_at.get(&key(d)).map(|&o| mentions(&decls[o].code));
        new_uses[r] = uses[r]
            .iter()
            .copied()
            .filter(|&q| match &old {
                Some(was) => !was.count.contains_key(decls[rows[q].decl].bare.as_str()),
                None => true,
            })
            .collect();
    }

    // Roots: nothing mentions them. A cycle has none, so whatever a root cannot
    // reach gets one of its own, the first by file and line.
    let mut root = vec![false; m];
    let mut reached = vec![false; m];
    let mut stack: Vec<usize> = vec![];
    for r in (0..m).filter(|&r| top(r) && indegree[r] == 0) {
        root[r] = true;
        stack.push(r);
    }
    loop {
        while let Some(r) = stack.pop() {
            if !std::mem::replace(&mut reached[r], true) {
                stack.extend(&uses[r]);
            }
        }
        let pick = (0..m)
            .filter(|&r| top(r) && !reached[r])
            .min_by_key(|&r| sort_key(r));
        let Some(pick) = pick else { break };
        root[pick] = true;
        stack.push(pick);
    }

    // Roots first, by file and line, then the rest the same way.
    let mut order: Vec<usize> = (0..m).filter(|&r| top(r)).collect();
    order.sort_by_cached_key(|&r| (!root[r], sort_key(r), r));

    // Each member under the definition it belongs to, in line order.
    let mut kids: HashMap<(Side, usize, &str), Vec<usize>> = HashMap::new();
    for r in (0..m).filter(|&r| !top(r)) {
        let d = &decls[rows[r].decl];
        kids.entry((d.side, d.file, d.parent.as_str()))
            .or_default()
            .push(r);
    }

    let id_of = |r: usize| {
        let d = &decls[rows[r].decl];
        format!("{}#{}", files[d.file].path, d.name)
    };
    let emit = |r: usize, out: &mut Vec<Row>| {
        let l = &rows[r];
        let d = &decls[l.decl];
        let base = old_at.get(&key(d)).map(|&o| &decls[o]);
        out.push(Row {
            path: files[d.file].path.clone(),
            kind: d.rule.clone(),
            name: d.name.clone(),
            class: d.parent.clone(),
            exported: exp[r],
            change: l.change.to_string(),
            side: if d.side == Side::Old { "old" } else { "new" }.to_string(),
            start: Some(d.start),
            end: Some(d.end),
            open: Some(l.open),
            added: Some(l.added),
            removed: Some(l.removed),
            head: d.head.clone(),
            base_start: base.map(|b| b.start),
            base_end: base.map(|b| b.end),
            staged: None,
            unstaged: None,
            uses: uses[r].iter().map(|&q| id_of(q)).collect(),
            root: Some(root[r]),
            new_uses: new_uses[r].iter().map(|&q| id_of(q)).collect(),
        });
    };
    let mut out: Vec<Row> = vec![];
    for &r in &order {
        emit(r, &mut out);
        let d = &decls[rows[r].decl];
        if let Some(list) = kids.get_mut(&(d.side, d.file, d.name.as_str())) {
            list.sort_by_key(|&q| decls[rows[q].decl].start);
            for &q in list.iter() {
                emit(q, &mut out);
            }
        }
    }

    // What no symbol claims: an import, a top-level statement. It gets a file row
    // after the symbols, with those lines alone counted. A blank line belongs to
    // nobody: the one after a new declaration is its separator.
    let mut claimed: HashSet<LineKey> = HashSet::new();
    for l in &rows {
        let d = &decls[l.decl];
        for line in d.start..=d.end {
            claimed.insert((d.side, d.file, line));
        }
        if d.side == Side::New {
            if let Some(&o) = old_at.get(&key(d)) {
                for line in decls[o].start..=decls[o].end {
                    claimed.insert((Side::Old, d.file, line));
                }
            }
        }
    }
    let mut listed: HashSet<String> = out.iter().map(|r| r.path.clone()).collect();
    let mut file_rows: Vec<Row> = vec![];
    for &i in &readable {
        let h = &hunks[i];
        let added = h
            .added()
            .filter(|(l, blank)| !blank && !claimed.contains(&(Side::New, i, *l)))
            .count() as u64;
        let removed = h
            .removed()
            .filter(|(l, blank)| !blank && !claimed.contains(&(Side::Old, i, *l)))
            .count() as u64;
        if added + removed > 0 {
            listed.insert(files[i].path.clone());
            file_rows.push(file_row(&files[i].path, "body", Some(added), Some(removed)));
        }
    }
    out.extend(file_rows);

    // Files this reads no symbols from, with the host's counts.
    let mut plain: Vec<Row> = vec![];
    for f in files {
        if !listed.contains(f.path.as_str()) {
            let change = match f.status {
                Status::Added | Status::Untracked => "added",
                Status::Removed => "removed",
                Status::Modified => "body",
            };
            plain.push(file_row(&f.path, change, f.added, f.removed));
        }
    }
    out.extend(plain);
    Ok(out)
}

fn file_row(path: &str, change: &str, added: Option<u64>, removed: Option<u64>) -> Row {
    Row {
        path: path.to_string(),
        kind: "file".to_string(),
        change: change.to_string(),
        side: "new".to_string(),
        added,
        removed,
        ..Row::default()
    }
}

/// The three versions of a file in the worktree: the base, the index and the disk.
pub struct Staged<'a> {
    pub base: &'a [u8],
    pub index: &'a [u8],
    pub disk: &'a [u8],
    pub untracked: bool,
}

/// What of each definition is staged and what is not, which is enough to stage
/// one function out of a file with three other things going on. Base lines are
/// known for a symbol the base has; an added one is found in the index by
/// shifting its worktree lines through the unstaged hunks above it.
pub fn mark_staged(rows: &mut [Row], versions: &HashMap<String, Staged<'_>>) {
    let mut hunks: HashMap<&str, (Hunks, Hunks)> = HashMap::new();
    for (path, v) in versions {
        hunks.insert(
            path,
            (
                Hunks::between(v.base, v.index),
                Hunks::between(v.index, v.disk),
            ),
        );
    }
    for r in rows.iter_mut() {
        if r.is_file() {
            continue;
        }
        let (Some(s), Some(e)) = (r.start, r.end) else {
            continue;
        };
        if versions.get(&r.path).is_some_and(|v| v.untracked) {
            r.staged = Some(false);
            r.unstaged = Some(true);
            continue;
        }
        let Some((ih, wi)) = hunks.get(r.path.as_str()) else {
            r.staged = Some(false);
            r.unstaged = Some(false);
            continue;
        };
        let (staged, unstaged) = if r.side == "old" {
            (
                ih.touched(Side::Old, s, e),
                wi.touched(Side::Old, ih.shift(Side::Old, s), ih.shift(Side::Old, e)),
            )
        } else {
            let unstaged = wi.touched(Side::New, s, e);
            let staged = match (r.base_start, r.base_end) {
                (Some(a), Some(b)) => ih.touched(Side::Old, a, b),
                _ => ih.touched(Side::New, wi.shift(Side::New, s), wi.shift(Side::New, e)),
            };
            (staged, unstaged)
        };
        r.staged = Some(staged);
        r.unstaged = Some(unstaged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<&str> {
        let m = mentions(text);
        let mut v: Vec<(&str, usize)> = m.first.iter().map(|(k, f)| (*k, *f)).collect();
        v.sort_by_key(|x| x.1);
        v.into_iter().map(|x| x.0).collect()
    }

    #[test]
    fn a_property_is_not_a_mention_but_its_object_is() {
        assert_eq!(names("runner.run()"), ["runner"]);
        assert_eq!(names("x = ...opts"), ["x", "opts"]);
    }

    #[test]
    fn a_key_is_not_a_mention() {
        assert_eq!(names("{ a: b }"), ["b"]);
        assert_eq!(names("{ a : b }"), ["b"]);
        assert_eq!(
            names("function f(o: Opts): string"),
            ["function", "f", "Opts", "string"]
        );
    }

    #[test]
    fn a_ternary_over_three_lines_mentions_both_arms() {
        assert_eq!(names("x\n  ? yes\n  : no"), ["x", "yes", "no"]);
    }

    #[test]
    fn a_table_round_trips_through_tsv() {
        let row = Row {
            path: "a.ts".into(),
            kind: "fn".into(),
            name: "run".into(),
            exported: Some(true),
            change: "body".into(),
            side: "new".into(),
            start: Some(1),
            end: Some(3),
            open: Some(1),
            added: Some(1),
            removed: Some(0),
            head: "function run()".into(),
            uses: vec!["a.ts#b".into(), "a.ts#c".into()],
            root: Some(true),
            ..Row::default()
        };
        assert_eq!(from_tsv(&to_tsv(std::slice::from_ref(&row))), vec![row]);
    }
}
