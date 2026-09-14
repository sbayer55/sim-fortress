//! Structural guard: no file under `src/` may exceed `MAX_LINES` lines.
//!
//! `clippy::too_many_lines` (denied at an 80-line threshold in `clippy.toml`)
//! measures *functions*, not files — a 2,700-line file made of small functions
//! passes it. Clippy has no file-length lint (rust-lang/rust-clippy#16674;
//! PR #15922 is unmerged), so this test walks `src/` itself.
//!
//! This began as a *ratchet*: an `OVER_BUDGET` allowlist grandfathered the nine
//! original offenders at their then-current size while the file-split refactor
//! (`docs/file-split-plan.md`) landed one file at a time. Every entry has since
//! been removed, so the allowlist is gone and the ceiling is absolute — a file
//! can no longer be granted an exemption, only split.

use std::path::{Path, PathBuf};

/// Hard ceiling for any file under `src/`.
const MAX_LINES: usize = 800;

#[test]
fn no_source_file_exceeds_the_ceiling() {
    for path in rust_sources() {
        let lines = line_count(&path);
        assert!(
            lines <= MAX_LINES,
            "{} is {lines} lines (limit {MAX_LINES}); split it into modules with a single purpose",
            relative(&path)
        );
    }
}

fn line_count(path: &Path) -> usize {
    let Ok(text) = std::fs::read_to_string(path) else {
        return 0;
    };
    text.lines().count()
}

/// Path relative to the crate root, with `/` separators on every platform.
fn relative(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every `.rs` file under `src/`, recursively.
fn rust_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
