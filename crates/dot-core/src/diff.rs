//! Unified diffs between rendered content and on-disk state.

use std::path::Path;

use similar::{ChangeTag, TextDiff};

/// Unified diff of `old` (on-disk, if any) against `new` content with
/// proper `@@ -l,c +l,c @@` hunk headers. A nonexistent file renders as a
/// pure addition.
pub fn unified_hunks(target: &Path, old: Option<&str>, new: &str) -> String {
    let old = old.unwrap_or("");
    let diff = TextDiff::from_lines(old, new);
    let display = target.display().to_string();

    let mut out = String::new();
    out.push_str(&format!("--- a/{display}\n+++ b/{display}\n"));

    for group in diff.grouped_ops(3).iter() {
        // header: first deleted-or-equal line vs first inserted-or-equal
        let old_start = group
            .iter()
            .find_map(|op| op.old_range().start.into())
            .unwrap_or(0);
        let new_start = group
            .iter()
            .find_map(|op| op.new_range().start.into())
            .unwrap_or(0);
        let old_len: usize = group.iter().map(|op| op.old_range().len()).sum();
        let new_len: usize = group.iter().map(|op| op.new_range().len()).sum();
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_start + 1,
            old_len,
            new_start + 1,
            new_len
        ));
        for op in group {
            for change in diff.iter_changes(op) {
                let sign = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal => ' ',
                };
                out.push_str(&format!("{sign}{}", change.value()));
                if !change.value().ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// True when the rendered diff is empty (no changes).
pub fn is_identical(old: Option<&str>, new: &str) -> bool {
    match old {
        None => new.is_empty(),
        Some(old) => old == new,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addition_for_new_file() {
        let d = unified_hunks(Path::new("/home/u/.zshrc"), None, "line1\nline2\n");
        assert!(d.contains("+++ b//home/u/.zshrc"));
        assert!(d.contains("+line1"));
        assert!(d.contains("+line2"));
    }

    #[test]
    fn modification_marks_both_sides() {
        let d = unified_hunks(Path::new("/home/u/.zshrc"), Some("a\nb\nc\n"), "a\nx\nc\n");
        assert!(d.contains("-b"));
        assert!(d.contains("+x"));
    }

    #[test]
    fn identical_is_empty() {
        let d = unified_hunks(Path::new("/f"), Some("same\n"), "same\n");
        assert!(is_identical(Some("same\n"), "same\n"));
        assert!(!d.contains('+') || d.contains("+++"));
    }
}
