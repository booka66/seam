//! The table as the graph a renderer draws: definitions, edges, and the files no
//! definition is read from. An edge carries its facet: a mention in the user's
//! own signature (`iface`) carries a change outward; one in its body stops there.
use crate::order::{self, Reading};
use crate::table::Row;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct Graph {
    pub rev: String,
    pub base: Option<String>,
    pub head: Option<String>,
    pub definitions: Vec<Definition>,
    /// What something outside seam says names what. seam-split fills it.
    pub references: Vec<serde_json::Value>,
    pub edges: Vec<Edge>,
    pub files: Vec<File>,
    pub reading: Reading,
}

#[derive(Serialize, Clone)]
pub struct Definition {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub name: String,
    pub class: Option<String>,
    pub exported: Option<bool>,
    pub change: String,
    pub side: String,
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub open: Option<u32>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub head: String,
    pub base: Option<Span>,
    pub staged: Option<bool>,
    pub unstaged: Option<bool>,
    pub root: bool,
    #[serde(rename = "box")]
    pub container: Container,
}

#[derive(Serialize, Clone)]
pub struct Span {
    pub start: Option<u32>,
    pub end: Option<u32>,
}

#[derive(Serialize, Clone)]
pub struct Container {
    pub file: String,
    pub kind: String,
}

#[derive(Serialize, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub facet: String,
    pub new: bool,
}

#[derive(Serialize, Clone)]
pub struct File {
    pub path: String,
    pub change: String,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    #[serde(rename = "box")]
    pub container: Container,
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Whether `text` names `name`: not inside a longer name, and not after a dot.
fn mentions(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(at) = text[from..].find(name) {
        let start = from + at;
        let end = start + name.len();
        let before = start == 0 || !(is_name_byte(bytes[start - 1]) || bytes[start - 1] == b'.');
        let after = end == bytes.len() || !is_name_byte(bytes[end]);
        if before && after {
            return true;
        }
        from = start + name.chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn name_of_id(id: &str) -> &str {
    let name = id.split_once('#').map_or(id, |(_, n)| n);
    name.strip_prefix("type ").unwrap_or(name)
}

pub fn build(
    rows: &[Row],
    kinds: &HashMap<String, String>,
    rev: &str,
    base: Option<String>,
    head: Option<String>,
) -> Graph {
    let container = |path: &str| Container {
        file: path.to_string(),
        kind: match kinds.get(path).map(String::as_str) {
            None | Some("") => "source".to_string(),
            Some(kind) => kind.to_string(),
        },
    };
    let defs: Vec<&Row> = rows.iter().filter(|r| !r.is_file()).collect();
    let definitions: Vec<Definition> = defs
        .iter()
        .map(|r| Definition {
            id: r.id(),
            path: r.path.clone(),
            kind: r.kind.clone(),
            name: r.name.clone(),
            class: (!r.class.is_empty()).then(|| r.class.clone()),
            exported: r.exported,
            change: r.change.clone(),
            side: r.side.clone(),
            start: r.start,
            end: r.end,
            open: r.open,
            added: r.added,
            removed: r.removed,
            head: r.head.clone(),
            base: r.base_start.map(|start| Span {
                start: Some(start),
                end: r.base_end,
            }),
            staged: r.staged,
            unstaged: r.unstaged,
            root: r.root == Some(true),
            container: container(&r.path),
        })
        .collect();
    let edges: Vec<Edge> = defs
        .iter()
        .flat_map(|r| {
            r.uses.iter().map(move |to| Edge {
                from: r.id(),
                to: to.clone(),
                facet: if mentions(&r.head, name_of_id(to)) {
                    "iface"
                } else {
                    "body"
                }
                .to_string(),
                new: r.new_uses.contains(to),
            })
        })
        .collect();
    let files: Vec<File> = rows
        .iter()
        .filter(|r| r.is_file())
        .map(|r| File {
            path: r.path.clone(),
            change: r.change.clone(),
            added: r.added,
            removed: r.removed,
            container: container(&r.path),
        })
        .collect();
    let reading = order::reading(&definitions, &edges);
    Graph {
        rev: rev.to_string(),
        base,
        head,
        definitions,
        references: vec![],
        edges,
        files,
        reading,
    }
}

#[cfg(test)]
mod tests {
    use super::mentions;

    #[test]
    fn a_name_is_mentioned_only_whole_and_not_after_a_dot() {
        assert!(mentions("function run(o: Opts)", "Opts"));
        assert!(!mentions("function run(o: OptsX)", "Opts"));
        assert!(!mentions("a.Opts", "Opts"));
        assert!(mentions("Opts.a", "Opts"));
    }
}
