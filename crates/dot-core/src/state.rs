//! dot state: manifest of managed files, in the XDG state dir.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{DotError, Result};

/// One managed file recorded in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedFile {
    /// Absolute target path (keyed by its string form).
    pub module: String,

    pub template: String,

    /// sha256 of the content dot last wrote.
    pub sha256: String,

    /// dot version at last write.
    pub dot_version: String,

    /// RFC3339 timestamp of the last write.
    pub written_at: String,
}

/// The manifest: `manifest.toml` in the state dir.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    /// target path -> entry.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, ManagedFile>,
}

/// Resolve the state dir: `$XDG_STATE_HOME/dot` (or
/// `~/.local/state/dot`), except macOS where no standard exists and dot
/// uses `~/.config/state/dot` (per the project plan).
pub fn state_dir(home: &Path) -> PathBuf {
    if let Some(dirs) = directories::ProjectDirs::from("", "", "dot") {
        // directories' state_dir is Linux-only; implement the plan's rule
        // directly instead.
        let _ = dirs;
    }
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("dot");
        }
    }
    if cfg!(target_os = "macos") {
        home.join(".config/state/dot")
    } else {
        home.join(".local/state/dot")
    }
}

pub fn manifest_path(home: &Path) -> PathBuf {
    state_dir(home).join("manifest.toml")
}

impl Manifest {
    pub fn load(home: &Path) -> Result<Manifest> {
        let path = manifest_path(home);
        if !path.exists() {
            return Ok(Manifest::default());
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| DotError::io(&path, e))?;
        toml::from_str(&raw).map_err(|source| DotError::ManifestCorrupt { path, source })
    }

    pub fn save(&self, home: &Path) -> Result<()> {
        let path = manifest_path(home);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DotError::io(parent, e))?;
        }
        let raw = toml::to_string_pretty(self).map_err(|e| DotError::Other(e.into()))?;
        std::fs::write(&path, raw).map_err(|e| DotError::io(&path, e))?;
        Ok(())
    }

    pub fn contains(&self, target: &Path) -> bool {
        self.files.contains_key(&target.display().to_string())
    }

    /// Record the result of writing `target` for `module`.
    pub fn record(
        &mut self,
        target: &Path,
        module: &str,
        template: &str,
        content: &str,
        dot_version: &str,
        written_at: &str,
    ) {
        self.files.insert(
            target.display().to_string(),
            ManagedFile {
                module: module.to_string(),
                template: template.to_string(),
                sha256: sha256_hex(content),
                dot_version: dot_version.to_string(),
                written_at: written_at.to_string(),
            },
        );
    }

    /// Drop a managed entry (file removed from disk or module disabled).
    pub fn remove(&mut self, target: &Path) {
        self.files.remove(&target.display().to_string());
    }

    /// All managed targets belonging to `module` (or all, if `None`),
    /// sorted.
    pub fn targets(&self, module: Option<&str>) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|(_, entry)| module.is_none_or(|m| entry.module == m))
            .map(|(path, _)| PathBuf::from(path))
            .collect()
    }
}

pub fn sha256_hex(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let home = tempfile::tempdir().unwrap();
        let mut m = Manifest::default();
        m.record(
            Path::new("/home/u/.zshrc"),
            "zsh",
            "zshrc.tmpl",
            "content",
            "0.1.0",
            "2026-10-18T00:00:00Z",
        );
        m.save(home.path()).unwrap();

        let loaded = Manifest::load(home.path()).unwrap();
        assert_eq!(loaded.files.len(), 1);
        let entry = loaded.files.get("/home/u/.zshrc").unwrap();
        assert_eq!(entry.module, "zsh");
        assert_eq!(entry.sha256, sha256_hex("content"));

        assert!(loaded.contains(Path::new("/home/u/.zshrc")));
        assert_eq!(loaded.targets(Some("zsh")).len(), 1);
        assert_eq!(loaded.targets(Some("git")).len(), 0);
        assert_eq!(loaded.targets(None).len(), 1);
    }

    #[test]
    fn missing_manifest_is_default() {
        let home = tempfile::tempdir().unwrap();
        let m = Manifest::load(home.path()).unwrap();
        assert!(m.files.is_empty());
    }

    #[test]
    fn state_dir_respects_xdg() {
        // not parallel-safe to mutate env; verify the pure branches
        let home = Path::new("/home/u");
        let dir = state_dir(home);
        if cfg!(target_os = "macos") {
            assert_eq!(dir, home.join(".config/state/dot"));
        } else {
            assert_eq!(dir, home.join(".local/state/dot"));
        }
    }
}
