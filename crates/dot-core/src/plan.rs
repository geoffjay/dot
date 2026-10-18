//! Write plans: what `dot write` (and `dot diff`) would do.

use std::path::{Path, PathBuf};

use crate::error::{DotError, Result};
use crate::render::RenderedFile;

/// One planned file action.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteAction {
    /// Target does not exist.
    Create,
    /// Target exists with different content.
    Update,
    /// Target exists with identical content (sha256 match).
    Unchanged,
}

/// A rendered file plus its computed action against the filesystem.
#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub rendered: RenderedFile,
    pub action: WriteAction,
    /// True when the target exists but is not in the manifest: dot would
    /// take ownership, saving the original first.
    pub adopting: bool,
}

#[derive(Debug, Clone, Default)]
pub struct WritePlan {
    pub files: Vec<PlannedFile>,
}

impl WritePlan {
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut create = 0;
        let mut update = 0;
        let mut unchanged = 0;
        for f in &self.files {
            match f.action {
                WriteAction::Create => create += 1,
                WriteAction::Update => update += 1,
                WriteAction::Unchanged => unchanged += 1,
            }
        }
        (create, update, unchanged)
    }
}

/// Compare the rendered files against on-disk state.
///
/// `managed` is the set of target paths currently in the manifest — used
/// to detect adoption of files dot does not own yet.
pub fn plan_write(
    rendered: Vec<RenderedFile>,
    managed: &dyn Fn(&Path) -> bool,
) -> Result<WritePlan> {
    let mut files = Vec::with_capacity(rendered.len());
    for r in rendered {
        let action = classify(&r)?;
        let adopting = action != WriteAction::Create && !managed(&r.target);
        files.push(PlannedFile {
            rendered: r,
            action,
            adopting,
        });
    }
    Ok(WritePlan { files })
}

fn classify(r: &RenderedFile) -> Result<WriteAction> {
    match std::fs::read(&r.target) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(WriteAction::Create),
        Err(err) => Err(DotError::io(&r.target, err)),
        Ok(existing) => {
            if existing == r.content.as_bytes() {
                Ok(WriteAction::Unchanged)
            } else {
                Ok(WriteAction::Update)
            }
        }
    }
}

/// Execute a plan: create parent dirs, back up adopted originals, write
/// files with their mode, and return the manifest entries to record.
///
/// This is the only place dot writes to `$HOME`.
pub fn execute_plan(
    plan: &WritePlan,
    force: bool,
    backup_adoption: &dyn Fn(&RenderedFile) -> Result<PathBuf>,
) -> Result<Vec<ExecutedFile>> {
    let mut executed = Vec::new();
    for planned in &plan.files {
        let r = &planned.rendered;
        if planned.action == WriteAction::Unchanged && !force {
            executed.push(ExecutedFile {
                target: r.target.clone(),
                action: WriteAction::Unchanged,
            });
            continue;
        }

        if let Some(parent) = r.target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DotError::io(parent, e))?;
        }

        if planned.adopting && r.target.exists() {
            let saved = backup_adoption(r)?;
            tracing::info!(target = %r.target.display(), saved = %saved.display(), "adopting unmanaged file");
        }

        std::fs::write(&r.target, &r.content).map_err(|e| DotError::io(&r.target, e))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&r.target, std::fs::Permissions::from_mode(r.mode))
                .map_err(|e| DotError::io(&r.target, e))?;
        }

        executed.push(ExecutedFile {
            target: r.target.clone(),
            action: if planned.action == WriteAction::Unchanged && force {
                // forced rewrite of identical content: still an Unchanged
                // classification for reporting
                WriteAction::Unchanged
            } else {
                planned.action.clone()
            },
        });
    }
    Ok(executed)
}

#[derive(Debug, Clone)]
pub struct ExecutedFile {
    pub target: PathBuf,
    pub action: WriteAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rendered_at(target: &Path, content: &str) -> RenderedFile {
        RenderedFile {
            module: "test".into(),
            target: target.to_path_buf(),
            template: "t".into(),
            content: content.to_string(),
            mode: 0o644,
        }
    }

    #[test]
    fn classify_create_update_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("exists.txt");
        std::fs::write(&existing, "same").unwrap();

        let cases = vec![
            (
                dir.path().join("new.txt"),
                "content",
                WriteAction::Create,
                false,
            ),
            (existing.clone(), "same", WriteAction::Unchanged, false),
            (existing.clone(), "different", WriteAction::Update, true),
        ];

        for (target, content, expected, _) in cases {
            let r = rendered_at(&target, content);
            let action = classify(&r).unwrap();
            assert_eq!(action, expected, "{:?}", target);
        }
    }

    #[test]
    fn adopting_detected_via_managed_set() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("owned-by-someone-else");
        std::fs::write(&f, "x").unwrap();

        let r = rendered_at(&f, "y");
        let plan = plan_write(vec![r], &|_| false).unwrap();
        assert!(plan.files[0].adopting);
        assert_eq!(plan.files[0].action, WriteAction::Update);

        let plan = plan_write(vec![rendered_at(&f, "y")], &|p| {
            p == dir.path().join("owned-by-someone-else")
        })
        .unwrap();
        assert!(!plan.files[0].adopting);
    }

    #[test]
    fn execute_writes_and_respects_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let new_file = dir.path().join("sub/new.txt");
        let r = rendered_at(&new_file, "hello");
        let plan = plan_write(vec![r], &|_| true).unwrap();

        let executed = execute_plan(&plan, false, &|_| {
            Err(DotError::Other(anyhow::anyhow!("no adoption expected")))
        })
        .unwrap();

        assert_eq!(executed.len(), 1);
        assert_eq!(std::fs::read_to_string(&new_file).unwrap(), "hello");
    }
}
