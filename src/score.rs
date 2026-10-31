//! How a change reads: not a judgement of the code, but a count of each thing
//! that makes a change harder to hold in your head, with the definition to blame,
//! so it can be argued with rather than trusted. The thresholds are what a
//! reviewer can carry, not measurements of anything, and they are all in the
//! `row` calls below. The verdict is the worst line, not an average, because a
//! change with one terrible dimension is hard to review however tidy the rest.
//!
//!   stories   entry points with something under them. Four stories in one branch
//!             is four reviews.
//!   loose     changed definitions nothing else changed touches: a sweep.
//!   knots     a cycle: an entry point something else mentions is one seam had to
//!             cut a cycle at.
//!   chain     the longest run a reader follows from one entry point, up to twelve.
//!   carry     changed interfaces something else names in its own signature, so
//!             the change does not stop there. The widest one is the blast.
//!   heaviest  the most changed lines in a single definition.
//!   spread    files and directories the change touches at all.
//!   safe      body-only changes, which nothing outside them can break. Never rated.
//!   breaks    callers outside the change that it breaks, when something has said
//!             so. Any at all is bad: the one line here that is a bug, not a burden.
use crate::graph::{Definition, Graph};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

#[derive(Serialize, Clone)]
pub struct ScoreRow {
    pub name: String,
    pub value: String,
    pub rating: u8,
    pub note: String,
    pub advice: String,
    pub n: i64,
    pub look: i64,
    pub bad: i64,
}

#[derive(Serialize, Clone)]
pub struct Totals {
    pub definitions: usize,
    pub edges: usize,
    pub files: usize,
}

#[derive(Serialize, Clone)]
pub struct Score {
    pub verdict: String,
    pub worst: u8,
    pub totals: Totals,
    pub rows: Vec<ScoreRow>,
}

/// A caller outside the change, as whatever found it said it.
#[derive(Deserialize, Default)]
pub struct Caller {
    pub from: String,
    #[serde(default)]
    pub breaks: bool,
}

fn row(
    name: &str,
    value: String,
    n: i64,
    look: i64,
    bad: i64,
    note: &str,
    advice: String,
) -> ScoreRow {
    let rating = if n >= bad {
        2
    } else if n >= look {
        1
    } else {
        0
    };
    ScoreRow {
        name: name.into(),
        value,
        rating,
        note: note.into(),
        advice,
        n,
        look,
        bad,
    }
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

pub fn score(g: &Graph, callers: Option<&[Caller]>) -> Score {
    let tops: Vec<&Definition> = g.definitions.iter().filter(|d| d.class.is_none()).collect();
    let edges: Vec<_> = g.edges.iter().filter(|e| e.from != e.to).collect();
    let mut out: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut into: HashMap<&str, usize> = HashMap::new();
    let mut in_signature: HashMap<&str, usize> = HashMap::new();
    for e in &edges {
        out.entry(&e.from).or_default().push(&e.to);
        *into.entry(&e.to).or_default() += 1;
        if e.facet == "iface" {
            *in_signature.entry(&e.to).or_default() += 1;
        }
    }
    let into_of = |d: &Definition| into.get(d.id.as_str()).copied().unwrap_or(0);
    let out_of = |d: &Definition| out.get(d.id.as_str()).map_or(0, Vec::len);
    let stories = tops
        .iter()
        .filter(|d| d.root && into_of(d) == 0 && out_of(d) > 0)
        .count();
    let loose = tops
        .iter()
        .filter(|d| into_of(d) == 0 && out_of(d) == 0)
        .count();
    let knots: Vec<&str> = tops
        .iter()
        .filter(|d| d.root && into_of(d) > 0)
        .map(|d| d.name.as_str())
        .collect();

    let ids: Vec<&str> = tops.iter().map(|d| d.id.as_str()).collect();
    let mut depth: HashMap<&str, i64> = ids.iter().map(|id| (*id, 1)).collect();
    for _ in 0..12 {
        let was = depth.clone();
        for id in &ids {
            let deepest = out.get(id).map_or(0, |list| {
                list.iter()
                    .map(|t| was.get(t).copied().unwrap_or(1) + 1)
                    .max()
                    .unwrap_or(0)
            });
            depth.insert(id, deepest.max(1));
        }
    }
    let chain = depth.values().copied().max().unwrap_or(0);

    let ifaces: Vec<(&str, usize)> = tops
        .iter()
        .filter(|d| d.change == "signature" || d.change == "removed")
        .map(|d| {
            (
                d.name.as_str(),
                in_signature.get(d.id.as_str()).copied().unwrap_or(0),
            )
        })
        .collect();
    let carry = ifaces.iter().filter(|(_, names)| *names > 0).count();
    // The first of the widest, the first of the heaviest: a stable sort's first.
    let widest = ifaces
        .iter()
        .copied()
        .reduce(|a, b| if b.1 > a.1 { b } else { a });
    let lines = |d: &Definition| d.added.unwrap_or(0) + d.removed.unwrap_or(0);
    let heavy = tops
        .iter()
        .copied()
        .reduce(|a, b| if lines(b) > lines(a) { b } else { a });
    let heavy_lines = heavy.map_or(0, lines);

    let paths: BTreeSet<&str> = g
        .definitions
        .iter()
        .map(|d| d.path.as_str())
        .chain(g.files.iter().map(|f| f.path.as_str()))
        .collect();
    let dirs: BTreeSet<&str> = paths
        .iter()
        .map(|p| p.rsplit_once('/').map_or(".", |(d, _)| d))
        .collect();
    let safe = tops.iter().filter(|d| d.change == "body").count();
    let n = tops.len();

    let mut rows = vec![
        row("stories", format!("{stories} to follow"), stories as i64, 4, 8, "entry points with something under them",
            format!("{stories} threads that nothing makes you read together. Split the branch at them and each half reads on its own.")),
        row("loose", format!("{loose} of {n} on their own"), (loose * 100 / n.max(1)) as i64, 40, 70, "changed definitions nothing else changed touches",
            "Mostly unrelated edits, so this is a sweep: read it as one pass over a list, not as a change.".to_string()),
        row("knots", if knots.is_empty() { "none".to_string() } else { format!("{} cut", knots.len()) }, knots.len() as i64, 1, 3,
            "definitions that depend on themselves, in a circle",
            format!("Cut at {}. Neither end can be read first, so untangle it before asking anyone to.", knots.join(", "))),
        row("chain", format!("{chain}{} deep", if chain >= 12 { "+" } else { "" }), chain, 6, 9, "definitions followed from one entry point",
            format!("The change threads through {chain} layers. Expect a reviewer to lose the thread before the end.")),
        row("carry", format!("{carry} of {} changed or gone", ifaces.len()), carry as i64, 4, 10, "interfaces something else names in its own signature",
            match widest {
                Some((name, names)) if names > 0 => format!("Widest is {name}, named in {names} other {}. Read those first: the change does not stop at them.", plural(names, "signature", "signatures")),
                _ => String::new(),
            }),
        row("heaviest", format!("{heavy_lines} lines in one"), heavy_lines as i64, 120, 300, "the most changed lines in a single definition",
            format!("{} changed {heavy_lines} lines. One definition that size is not reviewable as one diff.", heavy.map_or("nothing", |d| d.name.as_str()))),
        row("spread", format!("{} {}, {} {}", paths.len(), plural(paths.len(), "file", "files"), dirs.len(), plural(dirs.len(), "directory", "directories")),
            paths.len() as i64, 15, 40, "files the change touches at all", "Every file is a context switch, whether or not seam read definitions from it.".to_string()),
        row("safe", format!("{safe} of {n} body only"), 0, 1_000_000, 2_000_000, "nothing outside them can break because of them", String::new()),
    ];
    if let Some(callers) = callers {
        let broken: BTreeSet<&str> = callers
            .iter()
            .filter(|c| c.breaks)
            .map(|c| c.from.as_str())
            .collect();
        let all: BTreeSet<&str> = callers.iter().map(|c| c.from.as_str()).collect();
        let named: Vec<&str> = broken
            .iter()
            .take(3)
            .map(|f| f.rsplit_once('#').map_or(*f, |(_, n)| n))
            .collect();
        let more = if broken.len() > 3 {
            format!(" and {} more", broken.len() - 3)
        } else {
            String::new()
        };
        rows.push(row(
            "breaks",
            format!("{} of {} callers outside", broken.len(), all.len()),
            broken.len() as i64,
            1,
            1,
            "callers the change did not touch that it breaks",
            format!(
                "Breaks {}{more}. Fix them in this change, or say why they are fine.",
                named.join(", ")
            ),
        ));
    }
    let worst = rows.iter().map(|r| r.rating).max().unwrap_or(0);
    let verdict = if rows.iter().any(|r| r.name == "breaks" && r.rating == 2) {
        "breaks callers outside it"
    } else {
        match worst {
            2 => "hard to review as one change",
            1 => "worth reading in an order",
            _ => "reads as one thing",
        }
    };
    Score {
        verdict: verdict.to_string(),
        worst,
        totals: Totals {
            definitions: g.definitions.len(),
            edges: edges.len(),
            files: paths.len(),
        },
        rows,
    }
}

/// The verdict and any line not ok, in one line, for a footer or a CI comment.
pub fn brief(s: &Score) -> String {
    let flagged: Vec<String> = s
        .rows
        .iter()
        .filter(|r| r.rating > 0)
        .map(|r| format!("{} {}", r.name, r.value.split(' ').next().unwrap_or("")))
        .collect();
    if flagged.is_empty() {
        s.verdict.clone()
    } else {
        format!("{} · {}", s.verdict, flagged.join(" · "))
    }
}

struct Ink {
    reset: &'static str,
    bold: &'static str,
    dim: &'static str,
    rate: [&'static str; 3],
}

/// The score drawn for a terminal `width` wide: a bar in the colour of the rating,
/// the count, what it counts, and under any line not ok, what to do about it.
pub fn draw(s: &Score, rev: &str, width: usize, colour: bool) -> String {
    let ink = if colour {
        Ink {
            reset: "\x1b[0m",
            bold: "\x1b[1m",
            dim: "\x1b[2m",
            rate: ["\x1b[32m", "\x1b[33m", "\x1b[31m"],
        }
    } else {
        Ink {
            reset: "",
            bold: "",
            dim: "",
            rate: ["", "", ""],
        }
    };
    let Ink {
        reset: r,
        bold: b,
        dim: d,
        rate,
    } = ink;
    let bar = "▌";
    let mut out = String::from("\n");
    out += &format!(
        "  {b}seam{r}  {d}{}{r}\n",
        if rev.is_empty() { "a change" } else { rev }
    );
    out += &format!(
        "  {d}{} definitions · {} edges · {} files{r}\n\n",
        s.totals.definitions, s.totals.edges, s.totals.files
    );
    let fit = |t: &str, room: usize| -> String {
        if t.chars().count() > room {
            format!("{}…", t.chars().take(room - 1).collect::<String>())
        } else {
            t.to_string()
        }
    };
    let wrap = |text: &str, indent: &str, out: &mut String| {
        let room = width.min(104);
        let mut line = indent.to_string();
        for word in text.split_whitespace() {
            if line.chars().count() + word.chars().count() + 1 > room && line != indent {
                *out += &format!("{d}{line}{r}\n");
                line = indent.to_string();
            }
            if line != indent {
                line.push(' ');
            }
            line.push_str(word);
        }
        if line != indent {
            *out += &format!("{d}{line}{r}\n");
        }
    };
    for row in &s.rows {
        // A note cut to a stub says nothing: in a narrow pane it goes, and the
        // line under a flagged row carries the meaning instead.
        let room = width.saturating_sub(45);
        let note = if room >= 24 {
            format!(" {d}{}{r}", fit(&row.note, room))
        } else {
            String::new()
        };
        out += &format!(
            "  {}{bar}{r} {b}{:<9}{r} {:<28}{note}\n",
            rate[row.rating as usize], row.name, row.value
        );
        if row.rating > 0 && !row.advice.is_empty() {
            wrap(&row.advice, "      ", &mut out);
            out.push('\n');
        }
    }
    out += &format!("\n  {}{bar} {}{r}\n\n", rate[s.worst as usize], s.verdict);
    out
}
