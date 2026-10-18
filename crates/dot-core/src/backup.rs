//! Backups of dot-managed files under the state dir.
//!
//! `<state>/backups/<YYYYmmdd-HHMMSS>/` holds a full snapshot of managed
//! files plus the config and manifest at backup time. `<state>/backups/`
//! `pre-write/` holds originals of unmanaged files adopted by a write.

use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::config::Config;
use crate::error::{DotError, Result};
use crate::state::{manifest_path, Manifest};

pub type BackupId = String;

pub fn backups_dir(home: &Path) -> PathBuf {
    crate::state::state_dir(home).join("backups")
}

/// List snapshot ids (directory names), newest first.
pub fn list(home: &Path) -> Vec<BackupId> {
    let dir = backups_dir(home);
    let mut ids: Vec<BackupId> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter(|e| e.file_name() != "pre-write")
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids.reverse(); // YYYYmmdd-HHMMSS sorts chronologically
    ids
}

/// Create a timestamped snapshot of all dot-managed files (per the
/// manifest) plus config and manifest. Returns the snapshot id.
pub fn snapshot(
    home: &Path,
    manifest: &Manifest,
    _config: &Config,
    config_path: &Path,
    now: UtcNow,
) -> Result<BackupId> {
    let id = (now.f)();
    let dest = backups_dir(home).join(&id);
    let files_dest = dest.join("files");
    std::fs::create_dir_all(&files_dest).map_err(|e| DotError::io(&files_dest, e))?;

    for target in manifest.targets(None) {
        let Ok(relative) = target.strip_prefix(home) else {
            continue;
        };
        let copy_to = files_dest.join(relative);
        if let Some(parent) = copy_to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DotError::io(parent, e))?;
        }
        match std::fs::copy(&target, &copy_to) {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // managed file vanished from disk; snapshot what exists
                continue;
            }
            Err(err) => return Err(DotError::io(&target, err)),
        }
    }

    if config_path.exists() {
        let cfg_dest = dest.join("config.toml");
        std::fs::copy(config_path, &cfg_dest).map_err(|e| DotError::io(&cfg_dest, e))?;
    }
    let manifest_src = manifest_path(home);
    if manifest_src.exists() {
        let manifest_dest = dest.join("manifest.toml");
        std::fs::copy(&manifest_src, &manifest_dest)
            .map_err(|e| DotError::io(&manifest_dest, e))?;
    }

    Ok(id)
}

/// Injection point for the timestamp so tests are deterministic.
pub struct UtcNow {
    pub f: Box<dyn Fn() -> String>,
}

impl UtcNow {
    pub fn real() -> Self {
        Self {
            f: Box::new(|| Utc::now().format("%Y%m%d-%H%M%S").to_string()),
        }
    }
}

/// Keep at most `keep` snapshots, deleting the oldest beyond that.
pub fn prune_backups(home: &Path, keep: usize) -> Result<Vec<String>> {
    let ids = list(home);
    if ids.len() <= keep {
        return Ok(vec![]);
    }
    let to_delete: Vec<String> = ids[keep..].to_vec();
    for id in &to_delete {
        let dir = backups_dir(home).join(id);
        std::fs::remove_dir_all(&dir).map_err(|e| DotError::io(&dir, e))?;
    }
    Ok(to_delete)
}

/// Restore managed files from snapshot `id` (or the latest). Returns the
/// list of restored targets. Files that changed on disk after the backup
/// are refused unless `force`.
pub fn restore(home: &Path, id: Option<&str>, force: bool) -> Result<Vec<PathBuf>> {
    let ids = list(home);
    let id = match id {
        Some(id) => {
            if !ids.contains(&id.to_string()) {
                return Err(DotError::BackupNotFound(id.to_string()));
            }
            id.to_string()
        }
        None => ids
            .first()
            .ok_or(DotError::BackupNotFound("latest".into()))?
            .clone(),
    };

    let snapshot_files = backups_dir(home).join(&id).join("files");
    let backup_manifest: Manifest = {
        let path = backups_dir(home).join(&id).join("manifest.toml");
        if path.exists() {
            let raw = std::fs::read_to_string(&path).map_err(|e| DotError::io(&path, e))?;
            toml::from_str(&raw).map_err(|source| DotError::ManifestCorrupt { path, source })?
        } else {
            Manifest::default()
        }
    };

    let targets = backup_manifest.targets(None);
    if targets.is_empty() {
        // snapshot without manifest: restore whatever was captured
        return restore_walk(home, &snapshot_files, force);
    }

    let mut restored = Vec::new();
    for target in targets {
        let Ok(relative) = target.strip_prefix(home) else {
            continue;
        };
        let source = snapshot_files.join(relative);
        if !source.exists() {
            continue;
        }
        if target.exists() && !force {
            let current = std::fs::read(&target).map_err(|e| DotError::io(&target, e))?;
            let backup = std::fs::read(&source).map_err(|e| DotError::io(&source, e))?;
            if current != backup {
                return Err(DotError::Other(anyhow::anyhow!(
                    "{} changed on disk after the backup; use --force to overwrite",
                    target.display()
                )));
            }
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DotError::io(parent, e))?;
        }
        std::fs::copy(&source, &target).map_err(|e| DotError::io(&target, e))?;
        restored.push(target);
    }
    Ok(restored)
}

fn restore_walk(home: &Path, snapshot_files: &Path, force: bool) -> Result<Vec<PathBuf>> {
    let mut restored = Vec::new();
    for entry in walkdir::WalkDir::new(snapshot_files).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(snapshot_files) else {
            continue;
        };
        let target = home.join(relative);
        if target.exists() && !force {
            let current = std::fs::read(&target).map_err(|e| DotError::io(&target, e))?;
            let backup = std::fs::read(entry.path()).map_err(|e| DotError::io(entry.path(), e))?;
            if current != backup {
                return Err(DotError::Other(anyhow::anyhow!(
                    "{} changed on disk after the backup; use --force to overwrite",
                    target.display()
                )));
            }
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DotError::io(parent, e))?;
        }
        std::fs::copy(entry.path(), &target).map_err(|e| DotError::io(&target, e))?;
        restored.push(target);
    }
    Ok(restored)
}

/// Save an unmanaged original before a write adopts it, into
/// `backups/pre-write/`. Returns the saved path.
pub fn save_adoption(home: &Path, target: &Path, content: &[u8]) -> Result<PathBuf> {
    let relative = target
        .strip_prefix(home)
        .map_err(|_| DotError::TargetOutsideHome(target.to_path_buf()))?;
    let dest = backups_dir(home)
        .join("pre-write")
        .join(Utc::now().format("%Y%m%d-%H%M%S").to_string())
        .join(relative);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| DotError::io(parent, e))?;
    }
    std::fs::write(&dest, content).map_err(|e| DotError::io(&dest, e))?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_now(tag: &str) -> UtcNow {
        let tag = tag.to_string();
        UtcNow {
            f: Box::new(move || tag.clone()),
        }
    }

    fn setup(home: &Path) -> Manifest {
        let zshrc = home.join(".zshrc");
        std::fs::create_dir_all(home.join(".config")).unwrap();
        std::fs::write(&zshrc, "original").unwrap();
        std::fs::write(home.join(".config/config.toml"), "cfg").unwrap();

        let mut m = Manifest::default();
        m.record(&zshrc, "zsh", "zshrc.tmpl", "original", "0.1.0", "t0");
        m.save(home).unwrap();
        m
    }

    #[test]
    fn snapshot_and_restore_roundtrip() {
        let home = tempfile::tempdir().unwrap();
        let m = setup(home.path());
        let config: Config = toml::from_str("").unwrap();
        let config_path = home.path().join(".config/config.toml");

        let id = snapshot(
            home.path(),
            &m,
            &config,
            &config_path,
            fake_now("20260101-000000"),
        )
        .unwrap();
        assert_eq!(id, "20260101-000000");

        // mutate the managed file after the snapshot
        std::fs::write(home.path().join(".zshrc"), "changed").unwrap();

        // restore refuses without --force
        let err = restore(home.path(), None, false).unwrap_err();
        assert!(err.to_string().contains("--force"));

        let restored = restore(home.path(), None, true).unwrap();
        assert_eq!(restored, vec![home.path().join(".zshrc")]);
        assert_eq!(
            std::fs::read_to_string(home.path().join(".zshrc")).unwrap(),
            "original"
        );
    }

    #[test]
    fn restore_specific_id_and_missing() {
        let home = tempfile::tempdir().unwrap();
        let m = setup(home.path());
        let config: Config = toml::from_str("").unwrap();
        let config_path = home.path().join(".config/config.toml");

        snapshot(home.path(), &m, &config, &config_path, fake_now("a")).unwrap();
        std::fs::write(home.path().join(".zshrc"), "v2").unwrap();
        snapshot(home.path(), &m, &config, &config_path, fake_now("b")).unwrap();

        let ids = list(home.path());
        assert_eq!(ids, vec!["b".to_string(), "a".to_string()]);

        restore(home.path(), Some("a"), true).unwrap();
        assert_eq!(
            std::fs::read_to_string(home.path().join(".zshrc")).unwrap(),
            "original"
        );

        let err = restore(home.path(), Some("nope"), true).unwrap_err();
        assert!(matches!(err, DotError::BackupNotFound(_)));
    }

    #[test]
    fn prune_keeps_newest() {
        let home = tempfile::tempdir().unwrap();
        let m = setup(home.path());
        let config: Config = toml::from_str("").unwrap();
        let config_path = home.path().join(".config/config.toml");

        for tag in ["20260101-000001", "20260101-000002", "20260101-000003"] {
            snapshot(home.path(), &m, &config, &config_path, fake_now(tag)).unwrap();
        }

        let deleted = prune_backups(home.path(), 2).unwrap();
        assert_eq!(deleted, vec!["20260101-000001".to_string()]);
        assert_eq!(list(home.path()).len(), 2);
    }

    #[test]
    fn adoption_saved() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join(".zshrc");
        std::fs::write(&target, "precious").unwrap();
        let saved = save_adoption(home.path(), &target, b"precious").unwrap();
        assert!(saved.starts_with(backups_dir(home.path()).join("pre-write")));
        assert_eq!(std::fs::read_to_string(&saved).unwrap(), "precious");
    }

    #[test]
    fn snapshot_missing_files_skipped() {
        let home = tempfile::tempdir().unwrap();
        let mut m = Manifest::default();
        m.record(&home.path().join(".gone"), "zsh", "t", "x", "0.1.0", "t0");
        let config: Config = toml::from_str("").unwrap();
        let id = snapshot(
            home.path(),
            &m,
            &config,
            &home.path().join(".config/dot/config.toml"),
            fake_now("id"),
        )
        .unwrap();
        assert_eq!(id, "id");
    }
}
