//! seam in a browser: the core behind one call, with bytes in and JSON out. The
//! page reads a change's files however it can reach them and hands them over.
use crate::lang::Languages;
use crate::{graph, page, table, FileChange, Status, MAX_BYTES};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Input {
    rev: String,
    base: Option<String>,
    head: Option<String>,
    files: Vec<InputFile>,
}

/// One file of the change. `old` and `new` are its text on each side, which the
/// page may send for any file, and `added` and `removed` its line counts.
#[derive(Deserialize)]
struct InputFile {
    path: String,
    status: String,
    old: Option<String>,
    new: Option<String>,
    added: Option<u64>,
    removed: Option<u64>,
}

fn read(input: &str) -> Result<String, String> {
    let input: Input = serde_json::from_str(input).map_err(|e| format!("not a change: {e}"))?;
    let languages = Languages::new(vec![])?;
    let files: Vec<FileChange> = input
        .files
        .into_iter()
        .map(|f| FileChange {
            status: match f.status.as_str() {
                "added" => Status::Added,
                "removed" => Status::Removed,
                _ => Status::Modified,
            },
            // Only a file of a language seam reads has bytes, as on the command line,
            // so what the page chose to fetch cannot change what a file counts as.
            old: f.old.filter(|_| languages.reads(&f.path)).map(String::into_bytes),
            new: f.new.filter(|_| languages.reads(&f.path)).map(String::into_bytes),
            path: f.path,
            added: f.added,
            removed: f.removed,
        })
        .collect();
    let kinds: HashMap<String, String> = files
        .iter()
        .map(|f| (f.path.clone(), crate::kinds::kind_of(&f.path).to_string()))
        .collect();
    let rows = table::build(&files, &languages, &kinds, MAX_BYTES)?;
    let graph = graph::build(&rows, &kinds, &input.rev, input.base, input.head);
    let sides: HashMap<String, (Vec<u8>, Vec<u8>)> = files
        .into_iter()
        .map(|f| {
            (
                f.path,
                (f.old.unwrap_or_default(), f.new.unwrap_or_default()),
            )
        })
        .collect();
    let data = page::data(
        &graph,
        &page::diffs(&rows, &sides),
        &serde_json::Map::new(),
        None,
    );
    Ok(data.to_string())
}

// What the page calls: reserve room for the input, read it, free both. The answer
// is the page's data, or {"error": ...}, as four bytes of length then the JSON.

#[no_mangle]
pub extern "C" fn seam_reserve(n: usize) -> *mut u8 {
    let mut block = std::mem::ManuallyDrop::new(Vec::<u8>::with_capacity(n));
    block.as_mut_ptr()
}

/// # Safety
/// `p` is a block from `seam_reserve` or `seam_read`, `n` its size.
#[no_mangle]
pub unsafe extern "C" fn seam_free(p: *mut u8, n: usize) {
    drop(Vec::from_raw_parts(p, 0, n));
}

/// # Safety
/// `p` holds `n` bytes of JSON, from `seam_reserve`.
#[no_mangle]
pub unsafe extern "C" fn seam_read(p: *const u8, n: usize) -> *mut u8 {
    let input = std::slice::from_raw_parts(p, n);
    let answer = match std::str::from_utf8(input)
        .map_err(|e| e.to_string())
        .and_then(read)
    {
        Ok(json) => json,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    };
    let mut block = std::mem::ManuallyDrop::new(Vec::with_capacity(answer.len() + 4));
    block.extend_from_slice(&(answer.len() as u32).to_le_bytes());
    block.extend_from_slice(answer.as_bytes());
    block.as_mut_ptr()
}
