//! The file-access seam behind `include` resolution (SPEC §10.5) and
//! cross-file analysis. The `_from`
//! entry points read from disk through [`DiskVfs`], byte-identically to the
//! pre-seam behavior — error text included; the `_vfs` variants accept any
//! loader, e.g. a [`MemVfs`] file map for WASM hosts, embedders with
//! in-memory templates, or tests without tempdirs.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// File access for include resolution and analysis. Paths arriving here are
/// whatever [`crate::resolve`] built from the entry file and the authored
/// `include` paths — relative or absolute, possibly containing `.`/`..`.
pub trait Vfs {
    /// The file's source text, or an error message (it lands verbatim after
    /// `cannot include `path`: ` in compile errors).
    fn read(&self, path: &Path) -> Result<String, String>;

    /// The path's canonical identity, for include-cycle detection and dep
    /// lists. Two spellings of the same file must agree; a path that cannot
    /// be canonicalized falls back to itself (defensive — targets are read
    /// before identity matters).
    fn canon(&self, path: &Path) -> PathBuf;

    /// The canonical path if the target exists, `None` otherwise — what
    /// analysis stores as an include's resolution without reading the file.
    fn locate(&self, path: &Path) -> Option<PathBuf> {
        self.read(path).ok().map(|_| self.canon(path))
    }
}

/// The real filesystem — the default loader behind every `_from` entry
/// point.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskVfs;

impl Vfs for DiskVfs {
    fn read(&self, path: &Path) -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn canon(&self, path: &Path) -> PathBuf {
        fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn locate(&self, path: &Path) -> Option<PathBuf> {
        // Cheaper than the default: canonicalization already fails exactly
        // when the target doesn't exist — no read needed.
        fs::canonicalize(path).ok()
    }
}

/// An in-memory file map. Keys are lexically normalized (`.` dropped, `..`
/// popped — [`Vfs::canon`] can't touch a disk), so `partials/../head.fhtml`
/// and `head.fhtml` are the same file, mirroring how canonicalization
/// unifies spellings on disk.
#[derive(Debug, Clone, Default)]
pub struct MemVfs {
    files: BTreeMap<PathBuf, String>,
}

impl MemVfs {
    pub fn new() -> MemVfs {
        MemVfs::default()
    }

    /// Adds (or replaces) a file under the normalized form of `path`.
    pub fn add(&mut self, path: impl AsRef<Path>, src: impl Into<String>) -> &mut MemVfs {
        self.files.insert(normalize(path.as_ref()), src.into());
        self
    }
}

impl Vfs for MemVfs {
    fn read(&self, path: &Path) -> Result<String, String> {
        self.files
            .get(&normalize(path))
            .cloned()
            .ok_or_else(|| "no such file in the file map".to_string())
    }

    fn canon(&self, path: &Path) -> PathBuf {
        normalize(path)
    }
}

/// Lexical `.`/`..` resolution. `..` pops a preceding normal component and
/// otherwise survives (`../shared` from the map root stays `../shared` — the
/// caller chose the key space); a root component is never popped.
fn normalize(path: &Path) -> PathBuf {
    let mut out: Vec<Component> = Vec::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }
    out.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_is_lexical() {
        for (input, want) in [
            ("partials/../head.fhtml", "head.fhtml"),
            ("./partials/lib.fhtml", "partials/lib.fhtml"),
            ("a/./b/../c", "a/c"),
            ("../shared/x", "../shared/x"),
            ("a/../../x", "../x"),
            ("/a/../../x", "/x"),
        ] {
            assert_eq!(normalize(Path::new(input)), PathBuf::from(want), "{input}");
        }
    }

    #[test]
    fn mem_vfs_unifies_spellings() {
        let mut m = MemVfs::new();
        m.add("./head.fhtml", "p \"hi\"\n");
        assert!(m.read(Path::new("partials/../head.fhtml")).is_ok());
        assert_eq!(
            m.canon(Path::new("partials/../head.fhtml")),
            m.canon(Path::new("head.fhtml"))
        );
        assert_eq!(
            m.read(Path::new("nope.fhtml")).unwrap_err(),
            "no such file in the file map"
        );
        assert!(m.locate(Path::new("nope.fhtml")).is_none());
        assert_eq!(
            m.locate(Path::new("head.fhtml")),
            Some(PathBuf::from("head.fhtml"))
        );
    }
}
