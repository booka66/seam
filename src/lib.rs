//! seam's core: a change in; its definitions, their graph, its score and the
//! data of its page out. Nothing here starts a process or touches a file, so
//! the same code runs on a computer (src/main.rs brings the bytes from git)
//! and in a browser (src/wasm.rs takes them from whatever fetched them).

pub mod diff;
pub mod graph;
pub mod kinds;
pub mod lang;
pub mod order;
pub mod page;
pub mod score;
pub mod table;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// One file a change touched. `old` and `new` are its bytes on each side, there
/// only for a file of a language seam reads. `added` and `removed` are the
/// host's line counts, for a file no definition is read from.
pub struct FileChange {
    pub path: String,
    pub status: Status,
    pub old: Option<Vec<u8>>,
    pub new: Option<Vec<u8>>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Added,
    Removed,
    Modified,
    Untracked,
}

/// Files bigger than this list as plain files: one generated registry is tens
/// of megabytes of matches, and nobody reads a file that size by definition.
pub const MAX_BYTES: usize = 262_144;
