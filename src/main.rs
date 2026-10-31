//! The seam on your PATH: brings a change's bytes from git, finds what a repo
//! and a user have configured, hands it all to the core and prints what comes
//! back. Everything that starts a process or reads a file is here.
use seam::graph::{self, Graph};
use seam::lang::Languages;
use seam::table::{self, Row, Staged};
use seam::{page, score, FileChange, Status, MAX_BYTES};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "seam <rev>              what a change does, by definition rather than by file
seam <rev> --json       the same as a graph: definitions, edges, boxes
seam <rev> --json --diffs  with each definition's own diff in it
seam <rev> --html       that graph in a page you can click through, opened
seam <rev> --html --at <id>  the page open on that definition
seam <rev> --score      what makes the change hard to read, and how hard
seam <rev> --score --brief   the same in one line, for a footer
seam <rev> --split      how to split the change (seam-split)
any of them --table <file>   from a table already built, without reading the trees
any of them --with <file>    a JSON object merged into the graph: what else is known
seam --rules            the ast-grep rules behind what a definition is

<rev> is one commit, base..head, base...head or the word worktree.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure { message, code }) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            ExitCode::from(code)
        }
    }
}

struct Failure {
    message: String,
    code: u8,
}

fn fail<T>(message: impl Into<String>, code: u8) -> Result<T, Failure> {
    Err(Failure {
        message: message.into(),
        code,
    })
}

#[derive(Default)]
struct Args {
    rev: Option<String>,
    json: bool,
    diffs: bool,
    html: bool,
    score: bool,
    brief: bool,
    table: Option<String>,
    with: Option<String>,
    at: Option<String>,
}

fn parse_args(raw: &[String]) -> Result<Args, Failure> {
    let mut a = Args::default();
    let mut it = raw.iter();
    while let Some(arg) = it.next() {
        let mut value = |what: &str, flag: &str| match it.next() {
            Some(v) => Ok(v.clone()),
            None => fail(format!("seam: {flag} wants {what}"), 2),
        };
        match arg.as_str() {
            "--json" => a.json = true,
            "--diffs" => a.diffs = true,
            "--html" => a.html = true,
            "--score" => a.score = true,
            "--brief" => a.brief = true,
            "--table" => a.table = Some(value("a file", "--table")?),
            "--with" => a.with = Some(value("a file", "--with")?),
            "--at" => a.at = Some(value("a definition", "--at")?),
            other if other.starts_with('-') => return fail(format!("seam: no {other}"), 2),
            other => a.rev = Some(other.to_string()),
        }
    }
    Ok(a)
}

fn run() -> Result<(), Failure> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    // Splitting writes commits and runs what a repo asks it to, so it is a program
    // of its own beside this one, on what this one prints.
    if raw.iter().any(|a| {
        matches!(
            a.as_str(),
            "--split" | "--branch" | "--check" | "--plan-out"
        )
    }) {
        return exec_split(&raw);
    }
    match raw.first().map(String::as_str) {
        Some("--rules") => {
            print!("{}", languages(&top())?.rules_text());
            return Ok(());
        }
        Some("--version") => {
            println!("seam {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => {}
    }
    let args = parse_args(&raw)?;
    if let Some(t) = &args.table {
        if std::fs::metadata(t).is_err() {
            return fail(format!("seam: cannot read {t}"), 2);
        }
    }
    let Some(rev) = args.rev.clone() else {
        return fail(USAGE.trim_end(), 2);
    };
    let range = Range::resolve(&rev)?;
    let languages = languages(&range.root)?;
    let extras = read_extras(args.with.as_deref())?;

    let mut sides = Sides::new();
    let rows = match &args.table {
        Some(path) => table::from_tsv(
            &std::fs::read_to_string(path)
                .or_else(|_| fail(format!("seam: cannot read {path}"), 2))?,
        ),
        None => read_change(&range, &languages, &mut sides)?,
    };
    if !args.json && !args.score && !args.html {
        print_out(&table::to_tsv(&rows));
        return Ok(());
    }
    let kinds = kinds_of(&range.root, rows.iter().map(|r| r.path.clone()).collect());
    let graph = graph::build(
        &rows,
        &kinds,
        &rev,
        Some(range.base.clone()).filter(|b| !b.is_empty()),
        range.head.clone(),
    );

    if args.html {
        return draw_page(
            &rev,
            &range,
            &rows,
            &graph,
            &extras,
            args.at.as_deref(),
            &sides,
        );
    }
    if args.score {
        let callers = page::callers_in(&extras);
        let s = score::score(&graph, callers.as_deref());
        if args.brief {
            println!("{}", score::brief(&s));
        } else {
            print_out(&score::draw(&s, &rev, terminal_width(), wants_colour()));
        }
        return Ok(());
    }
    let value = if args.diffs {
        page::with_diffs(
            &graph,
            &page::diffs(&rows, &both_sides(&range, &rows, &sides)),
            &extras,
        )
    } else {
        page::with_extras(&graph, &extras)
    };
    print_out(&(value.to_string() + "\n"));
    Ok(())
}

fn print_out(text: &str) {
    // A closed pipe is the reader having what it wants.
    let _ = std::io::stdout().write_all(text.as_bytes());
}

fn exec_split(raw: &[String]) -> Result<(), Failure> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|e| e.canonicalize().ok())
        .and_then(|e| {
            let dir = e.parent()?.to_path_buf();
            // In a checkout the program is in target/release and seam-split in bin/.
            [dir.join("seam-split"), dir.join("../../bin/seam-split")]
                .into_iter()
                .find(|p| p.exists())
        });
    let program = beside.unwrap_or_else(|| PathBuf::from("seam-split"));
    let mut command = Command::new(&program);
    if let Ok(me) = std::env::current_exe() {
        command.env("SEAM_BIN", me);
    }
    match command.args(raw).status() {
        Ok(status) => {
            let code = status.code().unwrap_or(1);
            if code == 0 {
                Ok(())
            } else {
                fail("", code as u8)
            }
        }
        Err(_) => fail(
            "seam: --split wants seam-split, which is not beside seam or on your PATH",
            2,
        ),
    }
}

// --- what a repo and a user have configured

/// Where seam looks for what a repo tells it, nearest first: the repo's .seam,
/// this user's, then each directory in SEAM_PATH.
fn config_dirs(root: &Path) -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or(format!("{home}/.config"));
    let mut dirs = vec![root.join(".seam"), PathBuf::from(xdg).join("seam")];
    if let Ok(path) = std::env::var("SEAM_PATH") {
        dirs.extend(path.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    }
    dirs
}

fn languages(root: &Path) -> Result<Languages, Failure> {
    let mut files: Vec<(String, String)> = vec![];
    for dir in config_dirs(root) {
        let Ok(entries) = std::fs::read_dir(dir.join("languages")) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "yml"))
            .collect();
        paths.sort();
        for p in paths {
            let (Some(name), Ok(text)) = (
                p.file_name().and_then(|n| n.to_str()).map(str::to_string),
                std::fs::read_to_string(&p),
            ) else {
                continue;
            };
            files.push((name, text));
        }
    }
    Languages::new(files).or_else(|e| fail(format!("seam: {e}"), 2))
}

/// What each path is: SEAM_KINDS names a command for one run; else an executable
/// `kinds` where the language files are found; else seam's own reading of the path.
fn kinds_of(root: &Path, mut paths: Vec<String>) -> HashMap<String, String> {
    paths.sort();
    paths.dedup();
    let command = std::env::var("SEAM_KINDS")
        .ok()
        .filter(|c| !c.is_empty())
        .map(|c| {
            let mut sh = Command::new("sh");
            sh.args(["-c", &c]);
            sh
        });
    let command = command.or_else(|| {
        config_dirs(root)
            .into_iter()
            .map(|d| d.join("kinds"))
            .find(|f| is_executable(f))
            .map(|f| Command::new(f.canonicalize().unwrap_or(f)))
    });
    let Some(mut command) = command else {
        return paths
            .into_iter()
            .map(|p| {
                let kind = seam::kinds::kind_of(&p).to_string();
                (p, kind)
            })
            .collect();
    };
    let answer = feed(&mut command, paths.join("\n") + "\n").unwrap_or_default();
    String::from_utf8_lossy(&answer)
        .lines()
        .map(|l| {
            let (p, kind) = l.split_once('\t').unwrap_or((l, ""));
            (p.to_string(), kind.to_string())
        })
        .collect()
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

fn read_extras(path: Option<&str>) -> Result<Map<String, Value>, Failure> {
    let Some(path) = path else {
        return Ok(Map::new());
    };
    let text =
        std::fs::read_to_string(path).or_else(|_| fail(format!("seam: cannot read {path}"), 2))?;
    match serde_json::from_str(&text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => fail(format!("seam: {path} is not a JSON object"), 2),
    }
}

// --- git

fn git(args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

fn git_line(args: &[&str]) -> Option<String> {
    git(args).map(|o| String::from_utf8_lossy(&o).trim_end().to_string())
}

/// Run a command with `input` on its stdin and give back its stdout.
fn feed(command: &mut Command, input: String) -> Option<Vec<u8>> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    // On a thread of its own: a reader that answers as it reads would otherwise
    // fill its stdout while this is still writing.
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut out = vec![];
    child.stdout.take()?.read_to_end(&mut out).ok()?;
    let _ = writer.join();
    child.wait().ok()?;
    Some(out)
}

fn commit(name: &str) -> Result<String, Failure> {
    match git_line(&["rev-parse", "-q", "--verify", &format!("{name}^{{commit}}")]) {
        Some(sha) => Ok(sha),
        None => fail(format!("seam: {name} is not a commit"), 2),
    }
}

fn empty_tree() -> String {
    git_line(&["hash-object", "-t", "tree", "/dev/null"]).unwrap_or_default()
}

/// The way up to the repo's top, then each name's commit, in one git call. None
/// when a name does not resolve, for the caller to say which.
fn resolve_names(names: &[String]) -> Option<(PathBuf, Vec<String>)> {
    let mut args = vec!["rev-parse", "--show-cdup"];
    args.extend(names.iter().map(String::as_str));
    let out = String::from_utf8_lossy(&git(&args)?).into_owned();
    let mut lines: Vec<&str> = out.split('\n').collect();
    lines.pop();
    // A bare repo has no way up, and says nothing at all for it.
    let shas = lines.split_off(lines.len().checked_sub(names.len())?);
    let root = lines.first().filter(|up| !up.is_empty()).map_or(PathBuf::from("."), PathBuf::from);
    Some((root, shas.into_iter().map(str::to_string).collect()))
}

fn top() -> PathBuf {
    resolve_names(&[]).map_or(PathBuf::from("."), |(root, _)| root)
}

fn commit_name(name: &str) -> String {
    format!("{name}^{{commit}}")
}

/// The two trees. An end a range leaves out is HEAD, the way it is for git itself.
/// The worktree has no head commit: its files are read where they are.
struct Range {
    base: String,
    head: Option<String>,
    root: PathBuf,
}

impl Range {
    fn resolve(rev: &str) -> Result<Range, Failure> {
        if rev == "worktree" {
            return Ok(match resolve_names(&[commit_name("HEAD")]) {
                Some((root, shas)) => Range {
                    base: shas[0].clone(),
                    head: None,
                    root,
                },
                None => Range {
                    base: empty_tree(),
                    head: None,
                    root: top(),
                },
            });
        }
        if let Some((l, r)) = rev.split_once("...") {
            let (l, r) = (or_head(l), or_head(r));
            let (root, a, b) = both_ends(l, r)?;
            let Some(base) = git_line(&["merge-base", &a, &b]) else {
                return fail(format!("seam: {l} and {r} have no commit in common"), 2);
            };
            return Ok(Range {
                base,
                head: Some(b),
                root,
            });
        }
        if let Some((l, r)) = rev.split_once("..") {
            let (lname, rname) = (or_head(l), or_head(r));
            let (root, a, b) = both_ends(lname, rname)?;
            warn_if_two_dots_undo(rev, l, r, lname, rname, &a, &b);
            return Ok(Range {
                base: lname.to_string(),
                head: Some(rname.to_string()),
                root,
            });
        }
        if let Some((root, shas)) =
            resolve_names(&[commit_name(rev), commit_name(&format!("{rev}^"))])
        {
            return Ok(Range {
                base: shas[1].clone(),
                head: Some(rev.to_string()),
                root,
            });
        }
        // The first commit has no parent, and what is not a commit says so here.
        commit(rev)?;
        Ok(Range {
            base: empty_tree(),
            head: Some(rev.to_string()),
            root: top(),
        })
    }
}

fn both_ends(l: &str, r: &str) -> Result<(PathBuf, String, String), Failure> {
    match resolve_names(&[commit_name(l), commit_name(r)]) {
        Some((root, shas)) => Ok((root, shas[0].clone(), shas[1].clone())),
        None => {
            commit(l)?;
            commit(r)?;
            fail(format!("seam: cannot read {l} and {r}"), 2)
        }
    }
}

fn or_head(end: &str) -> &str {
    if end.is_empty() {
        "HEAD"
    } else {
        end
    }
}

/// Two dots are the two trees, which is only the branch when the left side is
/// behind the right. Once main has moved on, that diff also undoes every commit
/// main has and yours does not. git says nothing about this; a tool for reading a
/// change should.
fn warn_if_two_dots_undo(rev: &str, l: &str, r: &str, lname: &str, rname: &str, a: &str, b: &str) {
    let ancestor = Command::new("git")
        .args(["merge-base", "--is-ancestor", a, b])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !ancestor {
        let n = git_line(&["rev-list", "--count", &format!("{b}..{a}")]).unwrap_or_default();
        let s = if n == "1" { "" } else { "s" };
        eprintln!(
            "seam: {lname} is {n} commit{s} ahead of {rname}, so {rev} is both your change and the undoing of theirs. {}...{r} reads your change alone.",
            or_head(l)
        );
    }
}

/// One file in what `git diff --raw --numstat -z` says.
#[derive(Default, Clone)]
struct Entry {
    path: String,
    status: Option<Status>,
    old_sha: Option<String>,
    new_sha: Option<String>,
    added: Option<u64>,
    removed: Option<u64>,
}

const REGULAR: [&str; 2] = ["100644", "100755"];

/// The raw records then the numstat records, in one git call. A rename is split
/// back into the file that went and the file that came, its counts on the new
/// path, since a definition has no idea it moved.
fn parse_diff(output: &[u8]) -> Vec<Entry> {
    let mut fields = output
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned());
    let mut entries: HashMap<String, Entry> = HashMap::new();
    while let Some(field) = fields.next() {
        if let Some(raw) = field.strip_prefix(':') {
            let cols: Vec<&str> = raw.split(' ').collect();
            let [old_mode, new_mode, old_sha, new_sha, status] = cols[..] else {
                continue;
            };
            let regular = |mode: &str| REGULAR.contains(&mode);
            let letter = status.chars().next().unwrap_or('M');
            let first = fields.next().unwrap_or_default();
            let mut add = |path: String, status: Status, old: Option<&str>, new: Option<&str>| {
                let e = entries.entry(path.clone()).or_default();
                e.path = path;
                e.status = Some(status);
                e.old_sha = old.filter(|_| regular(old_mode)).map(str::to_string);
                e.new_sha = new.filter(|_| regular(new_mode)).map(str::to_string);
            };
            match letter {
                'R' | 'C' => {
                    let second = fields.next().unwrap_or_default();
                    if letter == 'R' {
                        add(first, Status::Removed, Some(old_sha), None);
                    }
                    add(second, Status::Added, None, Some(new_sha));
                }
                'A' => add(first, Status::Added, None, Some(new_sha)),
                'D' => add(first, Status::Removed, Some(old_sha), None),
                _ => add(first, Status::Modified, Some(old_sha), Some(new_sha)),
            }
        } else if let Some((added, rest)) = field.split_once('\t') {
            let Some((removed, path)) = rest.split_once('\t') else {
                continue;
            };
            // A rename writes an empty path, then the old path and the new.
            let path = if path.is_empty() {
                let _old = fields.next();
                fields.next().unwrap_or_default()
            } else {
                path.to_string()
            };
            let e = entries.entry(path.clone()).or_default();
            e.path = path;
            e.added = added.parse().ok();
            e.removed = removed.parse().ok();
        }
    }
    let mut list: Vec<Entry> = entries
        .into_values()
        .filter(|e| e.status.is_some())
        .collect();
    list.sort_by(|a, b| a.path.cmp(&b.path));
    list
}

/// Blobs by name, each read once, with one `git cat-file`.
fn cat_files(names: &[String]) -> HashMap<String, Vec<u8>> {
    let mut found = HashMap::new();
    if names.is_empty() {
        return found;
    }
    let mut command = Command::new("git");
    command.args(["cat-file", "--batch"]);
    let Some(out) = feed(&mut command, names.join("\n") + "\n") else {
        return found;
    };
    let mut at = 0;
    for name in names {
        let Some(eol) = out[at..].iter().position(|b| *b == b'\n') else {
            break;
        };
        let header = &out[at..at + eol];
        at += eol + 1;
        let size = header
            .rsplit(|b| *b == b' ')
            .next()
            .and_then(|s| std::str::from_utf8(s).ok())
            .and_then(|s| s.parse::<usize>().ok());
        if header.ends_with(b"missing") || header.ends_with(b"ambiguous") {
            continue;
        }
        let Some(size) = size else { break };
        found.insert(name.clone(), out[at..at + size].to_vec());
        at += size + 1;
    }
    found
}

/// Each file's bytes on both sides, as far as they have been read: what building
/// the table read, so a page does not ask again.
type Sides = HashMap<String, (Option<Vec<u8>>, Option<Vec<u8>>)>;

fn read_change(
    range: &Range,
    languages: &Languages,
    sides: &mut Sides,
) -> Result<Vec<Row>, Failure> {
    let entries = match &range.head {
        Some(head) => {
            let out = git(&[
                "diff",
                "--raw",
                "--numstat",
                "-z",
                "-M",
                "--abbrev=64",
                &range.base,
                head,
            ]);
            parse_diff(&out.unwrap_or_default())
        }
        None => {
            let out = git(&[
                "diff-index",
                "--raw",
                "--numstat",
                "-z",
                "-M",
                "--abbrev=64",
                &range.base,
            ]);
            let mut list = parse_diff(&out.unwrap_or_default());
            list.extend(untracked(&range.root));
            list.sort_by(|a, b| a.path.cmp(&b.path));
            list
        }
    };
    let max = std::env::var("SEAM_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(MAX_BYTES);
    let readable: Vec<&Entry> = entries
        .iter()
        .filter(|e| languages.reads(&e.path))
        .collect();
    let mut names: Vec<String> = readable
        .iter()
        .flat_map(|e| [e.old_sha.clone(), e.new_sha.clone()])
        .flatten()
        .filter(|s| s.bytes().any(|b| b != b'0'))
        .collect();
    names.sort();
    names.dedup();
    let blobs = cat_files(&names);
    let blob = |sha: &Option<String>| sha.as_ref().and_then(|s| blobs.get(s)).cloned();
    let disk = |path: &str| read_regular(&range.root.join(path));
    let mut files: Vec<FileChange> = vec![];
    for e in &entries {
        let status = e.status.unwrap_or(Status::Modified);
        let reads = languages.reads(&e.path);
        let old = if reads { blob(&e.old_sha) } else { None };
        let new = match (reads, &range.head, status) {
            (false, _, _) => None,
            (true, Some(_), _) => blob(&e.new_sha),
            (true, None, Status::Removed) => None,
            (true, None, _) => disk(&e.path),
        };
        sides.insert(e.path.clone(), (old.clone(), new.clone()));
        files.push(FileChange {
            path: e.path.clone(),
            status,
            old,
            new,
            added: e.added,
            removed: e.removed,
        });
    }
    let kinds = kinds_of(&range.root, files.iter().map(|f| f.path.clone()).collect());
    let mut rows =
        table::build(&files, languages, &kinds, max).or_else(|e| fail(format!("seam: {e}"), 1))?;
    if range.head.is_none() {
        mark_worktree(&mut rows, &files);
    }
    Ok(rows)
}

fn read_regular(path: &Path) -> Option<Vec<u8>> {
    std::fs::symlink_metadata(path)
        .ok()
        .filter(|m| m.is_file())?;
    std::fs::read(path).ok()
}

/// Untracked files from status, which the untracked cache makes fast where
/// ls-files --others walks the whole tree; it folds an untracked directory to its
/// name, so those few are walked.
fn untracked(root: &Path) -> Vec<Entry> {
    let top = root.to_string_lossy();
    let out = git(&["-C", &top, "status", "--porcelain=v1", "-z"]).unwrap_or_default();
    let mut paths = vec![];
    for record in out.split(|b| *b == 0) {
        let Some(p) = record.strip_prefix(b"?? ") else {
            continue;
        };
        let p = String::from_utf8_lossy(p).into_owned();
        if p.ends_with('/') {
            let inside = git(&[
                "-C",
                &top,
                "ls-files",
                "-z",
                "--others",
                "--exclude-standard",
                "--",
                &p,
            ])
            .unwrap_or_default();
            paths.extend(
                inside
                    .split(|b| *b == 0)
                    .filter(|f| !f.is_empty())
                    .map(|f| String::from_utf8_lossy(f).into_owned()),
            );
        } else {
            paths.push(p);
        }
    }
    paths
        .into_iter()
        .map(|path| Entry {
            path,
            status: Some(Status::Untracked),
            ..Entry::default()
        })
        .collect()
}

fn mark_worktree(rows: &mut [Row], files: &[FileChange]) {
    let mut paths: Vec<&str> = rows
        .iter()
        .filter(|r| !r.is_file())
        .map(|r| r.path.as_str())
        .collect();
    paths.sort_unstable();
    paths.dedup();
    let names: Vec<String> = paths
        .iter()
        .filter(|p| {
            files
                .iter()
                .any(|f| f.path == **p && f.status != Status::Untracked)
        })
        .map(|p| format!(":{p}"))
        .collect();
    let index = cat_files(&names);
    let empty: Vec<u8> = vec![];
    let mut versions: HashMap<String, Staged<'_>> = HashMap::new();
    for f in files.iter().filter(|f| paths.contains(&f.path.as_str())) {
        let untracked = f.status == Status::Untracked;
        versions.insert(
            f.path.clone(),
            Staged {
                base: f.old.as_ref().unwrap_or(&empty),
                index: index.get(&format!(":{}", f.path)).unwrap_or(&empty),
                disk: f.new.as_ref().unwrap_or(&empty),
                untracked,
            },
        );
    }
    table::mark_staged(rows, &versions);
}

/// Each definition's file on both sides: what the table was built from, or for a
/// table someone else built, read now.
fn both_sides(range: &Range, rows: &[Row], known: &Sides) -> HashMap<String, (Vec<u8>, Vec<u8>)> {
    let mut paths: Vec<&str> = rows
        .iter()
        .filter(|r| !r.is_file() && r.start.is_some())
        .map(|r| r.path.as_str())
        .collect();
    paths.sort_unstable();
    paths.dedup();
    let old_name = |p: &str| format!("{}:{p}", range.base);
    let head_name = |p: &str| range.head.as_ref().map(|h| format!("{h}:{p}"));
    let unread: Vec<&str> = paths
        .iter()
        .copied()
        .filter(|p| !known.contains_key(*p))
        .collect();
    let names: Vec<String> = unread
        .iter()
        .map(|p| old_name(p))
        .chain(unread.iter().filter_map(|p| head_name(p)))
        .collect();
    let blobs = cat_files(&names);
    paths
        .into_iter()
        .map(|p| {
            let (old, new) = match known.get(p) {
                Some((o, n)) => (o.clone(), n.clone()),
                None => (
                    blobs.get(&old_name(p)).cloned(),
                    match head_name(p) {
                        Some(name) => blobs.get(&name).cloned(),
                        None => read_regular(&range.root.join(p)),
                    },
                ),
            };
            (
                p.to_string(),
                (old.unwrap_or_default(), new.unwrap_or_default()),
            )
        })
        .collect()
}

// --- drawing

fn wants_colour() -> bool {
    match std::env::var("SEAM_COLOR").as_deref() {
        Ok("1" | "yes" | "always") => true,
        Ok("0" | "no" | "never") => false,
        _ => std::io::stdout().is_terminal(),
    }
}

/// COLUMNS first: a preview pane is narrower than the terminal it sits in, and it
/// is the caller who knows that. Then the terminal, asked through /dev/tty rather
/// than through stdout, which here is usually a pipe.
fn terminal_width() -> usize {
    let usable = |w: usize| (w >= 40).then_some(w);
    let from_env = std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .and_then(usable);
    let from_tty = || {
        let tty = std::fs::File::open("/dev/tty").ok()?;
        let out = Command::new("stty")
            .arg("size")
            .stdin(tty)
            .stderr(Stdio::null())
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
            .and_then(usable)
    };
    from_env.or_else(from_tty).unwrap_or(92).min(110)
}

/// A terminal on the other end has nothing to read a page with, so write it where
/// it can be opened and open it; a pipe or a file gets the page itself.
fn draw_page(
    rev: &str,
    range: &Range,
    rows: &[Row],
    graph: &Graph,
    extras: &Map<String, Value>,
    at: Option<&str>,
    sides: &Sides,
) -> Result<(), Failure> {
    let data = page::data(
        graph,
        &page::diffs(rows, &both_sides(range, rows, sides)),
        extras,
        at,
    );
    let html = page::html(&data);
    if !std::io::stdout().is_terminal() {
        print_out(&html);
        return Ok(());
    }
    let safe: String = rev
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let dir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
    let file = Path::new(&dir).join(format!("seam-{safe}.html"));
    std::fs::write(&file, html)
        .or_else(|e| fail(format!("seam: cannot write {}: {e}", file.display()), 1))?;
    eprintln!("{}", file.display());
    for opener in ["open", "xdg-open"] {
        let launched = Command::new(opener)
            .arg(&file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if launched.is_ok() {
            break;
        }
    }
    Ok(())
}
