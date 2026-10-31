//! What a definition is: language files of ast-grep rules, read by ast-grep's
//! own engine against the grammars compiled in here. A language file's `#seam:`
//! lines say which extensions it reads, which grammars its rules are written
//! against, what is not code in it (`noise`) and which of `:` and `?` make the
//! name before them a key (`keys`).
use ast_grep_config::{from_yaml_string, CombinedScan, GlobalRules, RuleConfig};
use ast_grep_core::language::Language as AstLanguage;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use regex::Regex;
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The language files seam ships, nearest first after anything a host adds.
pub const SHIPPED: [(&str, &str); 3] = [
    ("python.yml", include_str!("../share/languages/python.yml")),
    ("rust.yml", include_str!("../share/languages/rust.yml")),
    (
        "typescript.yml",
        include_str!("../share/languages/typescript.yml"),
    ),
];

/// The grammars compiled in. Adding one is a dependency and a line each here.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grammar {
    Typescript,
    Tsx,
    Rust,
    Python,
}

impl Grammar {
    fn for_extension(ext: &str) -> Option<Grammar> {
        match ext {
            "ts" | "mts" | "cts" => Some(Grammar::Typescript),
            "tsx" => Some(Grammar::Tsx),
            "rs" => Some(Grammar::Rust),
            "py" | "pyi" => Some(Grammar::Python),
            _ => None,
        }
    }
    // Where `$` cannot start an identifier the metavariable is written as this.
    fn expando(self) -> char {
        match self {
            Grammar::Rust | Grammar::Python => 'µ',
            _ => '$',
        }
    }
}

impl AstLanguage for Grammar {
    fn kind_to_id(&self, kind: &str) -> u16 {
        self.get_ts_language().id_for_node_kind(kind, true)
    }
    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.get_ts_language()
            .field_id_for_name(field)
            .map(|f| f.get())
    }
    fn expando_char(&self) -> char {
        self.expando()
    }
    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        if self.expando() == '$' {
            return Cow::Borrowed(query);
        }
        let mut out = String::with_capacity(query.len());
        let mut dollars = 0;
        for c in query.chars() {
            if c == '$' {
                dollars += 1;
                continue;
            }
            let sigil = if matches!(c, 'A'..='Z' | '_') || dollars == 3 {
                self.expando()
            } else {
                '$'
            };
            out.extend(std::iter::repeat_n(sigil, dollars));
            dollars = 0;
            out.push(c);
        }
        let sigil = if dollars == 3 { self.expando() } else { '$' };
        out.extend(std::iter::repeat_n(sigil, dollars));
        Cow::Owned(out)
    }
    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, *self))
    }
}

impl LanguageExt for Grammar {
    fn get_ts_language(&self) -> TSLanguage {
        match self {
            Grammar::Typescript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Grammar::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Grammar::Rust => tree_sitter_rust::LANGUAGE.into(),
            Grammar::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }
}

struct LanguageFile {
    name: String,
    text: String,
    extensions: Vec<String>,
    grammars: Vec<String>,
    noise: Option<Regex>,
    keys: Option<String>,
}

/// The languages a run reads, first of a name winning, first of an extension
/// winning. Rules compile on first use, since a run from a table needs none.
pub struct Languages {
    files: Vec<LanguageFile>,
    by_extension: HashMap<String, usize>,
    compiled: OnceLock<Result<Vec<RuleConfig<Grammar>>, String>>,
}

impl Languages {
    /// `files` are (name, text), nearest first; the ones seam ships come after.
    pub fn new(files: Vec<(String, String)>) -> Result<Languages, String> {
        let mut seen: Vec<String> = vec![];
        let mut parsed = vec![];
        let shipped = SHIPPED.iter().map(|(n, t)| (n.to_string(), t.to_string()));
        for (name, text) in files.into_iter().chain(shipped) {
            if seen.contains(&name) {
                continue;
            }
            seen.push(name.clone());
            parsed.push(LanguageFile::parse(name, text)?);
        }
        // Discovery order is what a repo's language file wins by, but the files
        // of one directory read in name order, as a shell glob has them.
        let mut by_extension = HashMap::new();
        for (i, f) in parsed.iter().enumerate() {
            for e in &f.extensions {
                by_extension
                    .entry(e.trim_start_matches('.').to_string())
                    .or_insert(i);
            }
        }
        Ok(Languages {
            files: parsed,
            by_extension,
            compiled: OnceLock::new(),
        })
    }

    /// The rules behind every language, one set a grammar, as ast-grep takes them.
    pub fn rules_text(&self) -> String {
        let mut out = String::new();
        let mut first = true;
        for f in &self.files {
            for g in &f.grammars {
                if !first {
                    out.push_str("---\n");
                }
                first = false;
                for line in f.text.split_inclusive('\n') {
                    if line.trim_end_matches('\n') == "language: LANG" {
                        out.push_str(&format!("language: {g}\n"));
                    } else {
                        out.push_str(line);
                    }
                }
            }
        }
        out
    }

    fn rules(&self) -> Result<&Vec<RuleConfig<Grammar>>, String> {
        self.compiled
            .get_or_init(|| {
                let text = self.rules_text();
                from_yaml_string::<Grammar>(&text, &GlobalRules::default()).map_err(|e| {
                    format!("the language rules did not read: {e}. seam has the grammars typescript, tsx, rust and python.")
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn file_of(&self, path: &str) -> Option<&LanguageFile> {
        self.by_extension
            .get(extension(path)?)
            .map(|i| &self.files[*i])
    }

    pub fn reads(&self, path: &str) -> bool {
        self.file_of(path).is_some()
    }

    /// Which language a path is, so a name is looked for among its own language.
    pub fn name_of(&self, path: &str) -> &str {
        self.file_of(path).map_or("", |f| &f.name)
    }

    /// A scanner per grammar, ready to read files from any thread.
    pub fn scanner(&self) -> Result<Scanner<'_>, String> {
        let rules = self.rules()?;
        Ok(Scanner {
            languages: self,
            rules,
        })
    }
}

impl LanguageFile {
    fn parse(name: String, text: String) -> Result<LanguageFile, String> {
        let (mut extensions, mut grammars, mut noise, mut keys) = (vec![], vec![], None, None);
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("#seam: ") else {
                continue;
            };
            let (key, value) = rest.split_once(' ').unwrap_or((rest, ""));
            let words = || {
                value
                    .split(' ')
                    .filter(|w| !w.is_empty())
                    .map(str::to_string)
            };
            match key {
                "extensions" => extensions = words().collect(),
                "grammars" => grammars = words().collect(),
                "noise" => {
                    noise = Some(Regex::new(value).map_err(|e| format!("{name}: noise: {e}"))?)
                }
                "keys" => keys = Some(value.to_string()),
                _ => {}
            }
        }
        Ok(LanguageFile {
            name,
            text,
            extensions,
            grammars,
            noise,
            keys,
        })
    }
}

fn extension(path: &str) -> Option<&str> {
    let file = path.rsplit('/').next()?;
    file.rsplit_once('.').map(|(_, e)| e)
}

/// One match of a language's rules in a file.
pub struct Match {
    pub rule: String,
    /// 1-based, the line the node starts on and the line it ends on.
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub head: String,
    /// The text as code, strings and comments blanked. Empty for a member.
    pub code: String,
}

pub struct Scanner<'l> {
    languages: &'l Languages,
    rules: &'l Vec<RuleConfig<Grammar>>,
}

impl Scanner<'_> {
    /// Every match in one file, grouped by rule and in order within a rule. A
    /// file that is not text, or that no grammar reads, has none.
    pub fn scan(&self, path: &str, source: &[u8]) -> Vec<Match> {
        let (Some(ext), Some(file)) = (extension(path), self.languages.file_of(path)) else {
            return vec![];
        };
        let Some(grammar) = Grammar::for_extension(ext) else {
            return vec![];
        };
        let Ok(source) = std::str::from_utf8(source) else {
            return vec![];
        };
        let rules: Vec<&RuleConfig<Grammar>> = self
            .rules
            .iter()
            .filter(|r| r.language == grammar)
            .collect();
        if rules.is_empty() {
            return vec![];
        }
        let scan = CombinedScan::new(rules);
        let Ok(doc) = StrDoc::try_new(source, grammar) else {
            return vec![];
        };
        let root = ast_grep_core::AstGrep::doc(doc);
        let result = scan.scan(&root, false);
        let mut out = vec![];
        let mut groups = result.matches;
        groups.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        for (rule, matches) in groups {
            for m in matches {
                let node = m.get_node();
                let range = node.range();
                let text = &source[range.clone()];
                let start = node.start_pos().line() as u32;
                // A node that takes its line's newline with it (a Rust // comment)
                // ends on the line before the one its range ends on.
                let end_line = node.end_pos().line() as u32;
                let end = if range.end > 0
                    && source.as_bytes()[range.end - 1] == b'\n'
                    && end_line > start
                {
                    end_line
                } else {
                    end_line + 1
                };
                let env = m.get_env();
                let body = env.get_match("B").map(|b| {
                    let r = b.range();
                    (r.start - range.start, r.end - range.start)
                });
                let id = rule.id.as_str();
                let head = match id {
                    "export" | "comment" => String::new(),
                    _ => clean(&raw_head(id, text, body)),
                };
                let name = match env.get_match("N") {
                    Some(n) => collapse_space(&n.text()),
                    None => collapse_space(&head),
                };
                let code = match id {
                    "export" | "comment" | "method" | "field" => String::new(),
                    _ => code_of(text, file),
                };
                out.push(Match {
                    rule: id.to_string(),
                    start: start + 1,
                    end,
                    name,
                    head,
                    code,
                });
            }
        }
        out
    }
}

/// The text before the body, its lines joined. A const or a type has no
/// signature to speak of, so it keeps the first line of its body.
fn raw_head(rule: &str, text: &str, body: Option<(usize, usize)>) -> String {
    let Some((b_start, b_end)) = body else {
        return match text.split_once('\n') {
            Some((first, _)) => format!("{first}…"),
            None => text.to_string(),
        };
    };
    let pre = text[..b_start].replace('\n', " ");
    if rule != "const" && rule != "type" {
        return pre;
    }
    let rest = &text[b_start..];
    let first = rest.split('\n').next().unwrap_or("");
    let more = if text[b_start..b_end].contains('\n') {
        "…"
    } else {
        ""
    };
    format!("{pre} {first}{more}")
}

/// Whitespace to single spaces, then the spaces a bracket or a comma does not
/// want, and an arrow at the end.
fn clean(head: &str) -> String {
    let squeezed = head.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = squeezed.chars().collect();
    let mut out = String::with_capacity(squeezed.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let drop =
            c == ' ' && (out.ends_with('(') || matches!(chars.get(i + 1), Some(',') | Some(')')));
        if !drop {
            out.push(c);
        }
        i += 1;
    }
    match out.trim_end().strip_suffix("=>") {
        Some(rest) => rest.trim_end().to_string(),
        None => out,
    }
}

fn collapse_space(s: &str) -> String {
    if s.contains(char::is_whitespace) {
        let mut out = String::with_capacity(s.len());
        let mut in_space = false;
        for c in s.chars() {
            if c.is_whitespace() {
                if !in_space {
                    out.push(' ');
                }
                in_space = true;
            } else {
                out.push(c);
                in_space = false;
            }
        }
        out
    } else {
        s.to_string()
    }
}

struct TsNoise {
    template: Regex,
    interpolation: Regex,
    rest: Regex,
}

fn ts_noise() -> &'static TsNoise {
    static NOISE: OnceLock<TsNoise> = OnceLock::new();
    NOISE.get_or_init(|| TsNoise {
        template: Regex::new(r"`((?:[^`\\]|\\.)*)`").expect("template"),
        interpolation: Regex::new(r"\$\{((?:[^{}]|\{[^{}]*\})*)\}").expect("interpolation"),
        rest: Regex::new(r#"/\*[\s\S]*?\*/|//[^\n]*|"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'"#)
            .expect("noise"),
    })
}

/// The code alone: a name in a string or a comment is not a use of it. A
/// language that says nothing reads like TypeScript, where a template literal
/// keeps its `${…}` interpolations, which are code, and loses the text between.
/// Then `a::b` names both, and whichever of `:` and `?` is not a key in this
/// language is only punctuation.
fn code_of(text: &str, file: &LanguageFile) -> String {
    let code = match &file.noise {
        Some(noise) => noise.replace_all(text, " ").into_owned(),
        None => {
            let n = ts_noise();
            let kept = n.template.replace_all(text, |c: &regex::Captures| {
                let inner: Vec<&str> = n
                    .interpolation
                    .captures_iter(&c[1])
                    .map(|i| i.get(1).map_or("", |m| m.as_str()))
                    .collect();
                format!(" {} ", inner.join(" "))
            });
            n.rest.replace_all(&kept, " ").into_owned()
        }
    };
    let code = code.replace("::", " ");
    let keys = file.keys.as_deref().unwrap_or(":?");
    code.chars()
        .map(|c| {
            if (c == ':' || c == '?') && !keys.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_head_loses_the_spaces_a_bracket_does_not_want() {
        assert_eq!(
            clean("function  run( a: b , c )\n"),
            "function run(a: b, c)"
        );
        assert_eq!(
            clean("const fresh = (o: Opts) => "),
            "const fresh = (o: Opts)"
        );
    }

    #[test]
    fn the_shipped_rules_read_and_find_definitions() {
        let langs = Languages::new(vec![]).unwrap();
        let scanner = langs.scanner().unwrap();
        let found = scanner.scan(
            "a.ts",
            b"export function run(o: Opts): number {\n  return 1;\n}\n",
        );
        let fns: Vec<_> = found.iter().filter(|m| m.rule == "fn").collect();
        assert_eq!(fns[0].name, "run");
        assert_eq!(fns[0].head, "function run(o: Opts): number");
        assert_eq!((fns[0].start, fns[0].end), (1, 3));
        assert!(found.iter().any(|m| m.rule == "export"));
    }

    #[test]
    fn rust_and_python_patterns_bind_their_names() {
        let langs = Languages::new(vec![]).unwrap();
        let scanner = langs.scanner().unwrap();
        assert!(scanner
            .scan("a.rs", b"pub fn run() -> u32 { 1 }\n")
            .iter()
            .any(|m| m.rule == "fn" && m.name == "run"));
        assert!(scanner
            .scan("a.py", b"def run():\n    return 1\n")
            .iter()
            .any(|m| m.rule == "fn" && m.name == "run"));
    }
}
