//! Structural guard: no file under `src/` may exceed `MAX_LINES` lines.
//!
//! `clippy::too_many_lines` (denied at an 80-line threshold in `clippy.toml`)
//! measures *functions*, not files — a 2,700-line file made of small functions
//! passes it today. Clippy has no file-length lint
//! (rust-lang/rust-clippy#16674; PR #15922 is unmerged), so this test walks
//! `src/` itself.
//!
//! It is a *ratchet*. `OVER_BUDGET` records every file still above `MAX_LINES`
//! together with the size it had when the guard landed. The list may only
//! shrink:
//!
//! * a file at or below `MAX_LINES` must be removed from `OVER_BUDGET`
//!   (enforced by `budget_list_is_not_stale`), and
//! * no file may exceed the budget it is given.
//!
//! See `docs/file-split-plan.md` for the wave-by-wave breakdown.

use std::path::{Path, PathBuf};

/// Hard ceiling for any file under `src/`.
const MAX_LINES: usize = 800;

/// Grandfathered files: `(path relative to the crate root, line budget)`.
///
/// Kept sorted by path. Delete an entry as soon as the file is at or below
/// [`MAX_LINES`] — leaving it behind fails `budget_list_is_not_stale`.
const OVER_BUDGET: &[(&str, usize)] = &[
    ("src/sim/behavior.rs", 2721),
    ("src/sim/disease.rs", 1561),
    ("src/sim/params.rs", 1308),
    ("src/ui/screens/s01_map.rs", 2495),
];

#[test]
fn no_source_file_exceeds_its_budget() {
    for path in rust_sources() {
        let rel = relative(&path);
        let budget = budget_for(&rel);
        let lines = line_count(&path);
        assert!(
            lines <= budget,
            "{rel} is {lines} lines (budget {budget}); split it into modules with a single purpose"
        );
    }
}

#[test]
fn budget_list_is_not_stale() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (rel, budget) in OVER_BUDGET {
        let path = root.join(rel);
        assert!(
            path.is_file(),
            "{rel} is listed in OVER_BUDGET but no longer exists; remove the entry"
        );
        let lines = line_count(&path);
        assert!(
            lines <= *budget,
            "{rel} grew from {budget} to {lines} lines; split it instead of raising the budget"
        );
        assert!(
            lines > MAX_LINES,
            "{rel} is now {lines} lines (<= {MAX_LINES}); remove its OVER_BUDGET entry"
        );
    }
}

#[test]
fn budget_list_is_sorted() {
    let mut sorted = OVER_BUDGET.to_vec();
    sorted.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
    assert_eq!(
        OVER_BUDGET, sorted,
        "OVER_BUDGET must stay sorted by path"
    );
}

/// Budget for `rel`: its `OVER_BUDGET` entry, else the hard ceiling.
fn budget_for(rel: &str) -> usize {
    OVER_BUDGET
        .iter()
        .find(|(path, _)| *path == rel)
        .map_or(MAX_LINES, |(_, budget)| *budget)
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
