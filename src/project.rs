//! Project root resolution shared by both strategies. Follows the Typst CLI: the root is
//! `--root`, then `TYPST_ROOT` (handled by clap), then the entry file's directory. The entry
//! file must lie inside the root, and import, include and file paths resolve against it.

use std::fs;
use std::path::{Path, PathBuf};

use typst_syntax::VirtualPath;

pub struct Project {
    /// Canonical directory that bounds every file the extraction may read.
    pub root: PathBuf,
    /// The entry file, relative to the root.
    pub main: VirtualPath,
}

impl Project {
    /// Same defaults as the Typst CLI: an explicit root (from `--root`, or `TYPST_ROOT` through
    /// clap), else the directory of the canonical entry file, else the current directory.
    pub fn locate(entry: &Path, root: Option<&Path>) -> Result<Self, String> {
        let canonical = fs::canonicalize(entry).map_err(|e| format!("error: {}: {e}", entry.display()))?;
        let default = canonical.parent().unwrap_or(Path::new("."));
        let root_arg = root.unwrap_or(default);
        let root = fs::canonicalize(root_arg)
            .map_err(|e| format!("error: root {}: {e}", root_arg.display()))?;
        let main = VirtualPath::virtualize(&root, &canonical).map_err(|_| {
            format!(
                "error: {} is outside the project root {} (set --root or TYPST_ROOT)",
                entry.display(),
                root_arg.display()
            )
        })?;
        Ok(Self { root, main })
    }
}
