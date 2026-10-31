//! The page: `share/view.html` with the graph inside it, one self-contained file
//! that works offline and can be sent to someone. Each definition carries its own
//! diff, so the data is the graph plus what a page alone can't work out.
use crate::diff::walk;
use crate::graph::Graph;
use crate::score::{score, Caller};
use crate::table::Row;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

const VIEW: &str = include_str!("../share/view.html");
const MARKER: &str = "<!--SEAM DATA-->";
/// A definition this long is not read by the line, and a page holding it would be.
const MAX_LINES: usize = 400;

#[derive(Serialize, Clone)]
pub struct DiffLine {
    pub mark: String,
    pub old: String,
    pub new: String,
    pub text: String,
}

/// Each definition's own patch: of the whole file's diff, the lines it has on the
/// new side and the lines it had on the old. Taking the two sides separately is
/// what keeps a removed definition's lines out of the definition that followed it.
/// `sides` holds each file's (old, new) bytes.
pub fn diffs(
    rows: &[Row],
    sides: &HashMap<String, (Vec<u8>, Vec<u8>)>,
) -> HashMap<String, Vec<DiffLine>> {
    struct Span {
        id: String,
        new: Option<(u32, u32)>,
        old: Option<(u32, u32)>,
    }
    let mut by_file: HashMap<&str, Vec<Span>> = HashMap::new();
    for r in rows.iter().filter(|r| !r.is_file()) {
        let (Some(s), Some(e)) = (r.start, r.end) else {
            continue;
        };
        let (new, old) = if r.side == "new" {
            (Some((s, e)), r.base_start.zip(r.base_end))
        } else {
            (None, Some((s, e)))
        };
        by_file.entry(&r.path).or_default().push(Span {
            id: r.id(),
            new,
            old,
        });
    }
    let mut out: HashMap<String, Vec<DiffLine>> = HashMap::new();
    let inside = |range: Option<(u32, u32)>, line: Option<u32>| match (range, line) {
        (Some((a, z)), Some(l)) => a <= l && l <= z,
        _ => false,
    };
    for (path, spans) in by_file {
        let Some((old, new)) = sides.get(path) else {
            continue;
        };
        walk(old, new, |line| {
            for span in &spans {
                if inside(span.new, line.new) || inside(span.old, line.old) {
                    let lines = out.entry(span.id.clone()).or_default();
                    if lines.len() < MAX_LINES {
                        lines.push(DiffLine {
                            mark: line.mark.to_string(),
                            old: line.old.map_or(String::new(), |l| l.to_string()),
                            new: line.new.map_or(String::new(), |l| l.to_string()),
                            text: String::from_utf8_lossy(line.text).into_owned(),
                        });
                    }
                }
            }
        });
    }
    out
}

/// The graph as JSON with `with` merged over it: what something else knows and
/// seam cannot (callers, notes), for every renderer to carry.
pub fn with_extras(graph: &Graph, with: &Map<String, Value>) -> Value {
    let mut value = serde_json::to_value(graph).expect("a graph is plain data");
    if let Value::Object(map) = &mut value {
        for (k, v) in with {
            map.insert(k.clone(), v.clone());
        }
    }
    value
}

pub fn callers_in(with: &Map<String, Value>) -> Option<Vec<Caller>> {
    with.get("callers")
        .and_then(|c| serde_json::from_value(c.clone()).ok())
}

/// The graph with each definition's own diff in it, and `with` merged over it.
pub fn with_diffs(
    graph: &Graph,
    diffs: &HashMap<String, Vec<DiffLine>>,
    with: &Map<String, Value>,
) -> Value {
    let mut value = with_extras(graph, with);
    if let Some(Value::Array(defs)) = value.get_mut("definitions") {
        for d in defs {
            let Value::Object(def) = d else { continue };
            let lines = def
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| diffs.get(id));
            def.insert(
                "diff".into(),
                serde_json::to_value(lines).expect("diff lines are plain data"),
            );
        }
    }
    value
}

/// The data a page draws: the graph, each definition's diff, its score.
pub fn data(
    graph: &Graph,
    diffs: &HashMap<String, Vec<DiffLine>>,
    with: &Map<String, Value>,
    at: Option<&str>,
) -> Value {
    let mut value = with_diffs(graph, diffs, with);
    let callers = callers_in(with);
    if let Value::Object(map) = &mut value {
        map.insert(
            "score".into(),
            serde_json::to_value(score(graph, callers.as_deref())).expect("a score is plain data"),
        );
        if let Some(at) = at {
            map.insert("at".into(), Value::String(at.to_string()));
        }
    }
    value
}

/// The whole page. `</` in the data is escaped so no string in the code being
/// read can close the script tag early.
pub fn html(data: &Value) -> String {
    let json = data.to_string().replace("</", "<\\/");
    let (before, after) = VIEW.split_once(MARKER).expect("the page has its marker");
    // The marker's line stays out of the page, as the rest of it reads on its own.
    let before = before.rsplit_once('\n').map_or(before, |(b, _)| b);
    let after = after.split_once('\n').map_or(after, |(_, a)| a);
    format!(
        "{before}\n<script id=\"seam-data\" type=\"application/json\">{json}\n</script>\n{after}"
    )
}
