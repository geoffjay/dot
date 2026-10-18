//! Module definitions embedded in the binary.
//!
//! A module is `modules/<name>/` with `module.toml` (metadata, defaults,
//! file targets) plus templates under `modules/<name>/templates/`.

use std::collections::BTreeMap;

use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};

use crate::error::{DotError, Result};

pub static MODULES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../modules");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub name: String,

    #[serde(default)]
    pub description: String,

    /// Supported OSes (values of `std::env::consts::OS`); empty = all.
    #[serde(default)]
    pub os: Vec<String>,

    /// Default module settings, merged under the config layers.
    #[serde(default = "default_toml_table")]
    pub defaults: toml::Value,

    /// template name -> target path (may be `{ path = "...", mode = 0o755 }`).
    #[serde(default)]
    pub files: BTreeMap<String, FileTarget>,

    /// Per-OS file additions/overrides (same shape as `files`).
    #[serde(default)]
    pub files_os: BTreeMap<String, BTreeMap<String, FileTarget>>,

    /// Target paths that must never appear in propose output.
    #[serde(default)]
    pub sensitive: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileTarget {
    Path(String),
    Detailed {
        path: String,
        #[serde(default = "default_mode")]
        mode: u32,
    },
}

fn default_mode() -> u32 {
    0o644
}

fn default_toml_table() -> toml::Value {
    toml::Value::Table(Default::default())
}

impl FileTarget {
    pub fn path(&self) -> &str {
        match self {
            FileTarget::Path(p) => p,
            FileTarget::Detailed { path, .. } => path,
        }
    }

    pub fn mode(&self) -> u32 {
        match self {
            FileTarget::Path(_) => 0o644,
            FileTarget::Detailed { mode, .. } => *mode,
        }
    }
}

impl Module {
    /// Parse `module.toml` from the embedded dir entry.
    pub fn parse(raw: &str) -> Result<Module> {
        // module.toml uses `os = [...]`, `defaults`, `files`, and optional
        // `[files.linux]` tables. `[files.<os>]` is lifted into `files_os`
        // before deserializing so `files` stays a flat map.
        let mut dynamic: toml::Table = toml::from_str(raw)
            .map_err(|e| DotError::Other(anyhow::anyhow!("module.toml: {e}")))?;

        let mut files = match dynamic.remove("files") {
            Some(toml::Value::Table(t)) => t,
            Some(v) => {
                return Err(DotError::Other(anyhow::anyhow!(
                    "module.toml: `files` must be a table, got {v}"
                )))
            }
            None => Default::default(),
        };

        let mut files_os = BTreeMap::new();
        let keys: Vec<String> = files.keys().cloned().collect();
        for key in keys {
            // a nested table under [files] with known OS names is a
            // per-OS file map, not a template entry
            if let Some(toml::Value::Table(inner)) = files.get(&key) {
                let is_os_map = inner.keys().all(|k| k == "path" || k == "mode")
                    && !inner.contains_key("path")
                    || is_os_name(&key);
                if is_os_map {
                    let value = files.remove(&key).expect("key just checked");
                    files_os.insert(key, table_to_targets(value)?);
                }
            }
        }

        // Re-deserialize with the `files` table removed: it is replaced
        // by the lifted files/files_os computed above. Deserializing the
        // raw text again would fail on [files.<os>] subtables.
        dynamic.remove("files");
        let mut module: Module = toml::from_str(&toml::to_string(&dynamic).unwrap_or_default())
            .map_err(|e| DotError::Other(anyhow::anyhow!("module.toml: {e}")))?;
        module.files = table_to_targets(toml::Value::Table(files))?;
        module.files_os = files_os;
        Ok(module)
    }

    /// All embedded modules, sorted by name.
    pub fn all() -> Vec<Module> {
        let mut modules: Vec<Module> = MODULES
            .dirs()
            .filter(|d| {
                d.path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_none_or(|n| !n.starts_with('.'))
            })
            .filter_map(|d| {
                // include_dir resolves paths relative to the embedded
                // root, so look up "<name>/module.toml" on the root Dir.
                let meta_path = format!("{}/module.toml", d.path().display());
                let raw = MODULES
                    .get_file(&meta_path)
                    .and_then(|f| f.contents_utf8())?;
                Module::parse(raw).ok().map(|mut m| {
                    // trust the directory name over the metadata field
                    m.name = d
                        .path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_string();
                    m
                })
            })
            .collect();
        modules.sort_by(|a, b| a.name.cmp(&b.name));
        modules
    }

    /// Look up an embedded module by name.
    pub fn by_name(name: &str) -> Result<Module> {
        Self::all()
            .into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| DotError::ModuleNotFound(name.to_string()))
    }

    /// Effective file targets for the given host OS.
    pub fn files_for_os(&self, os: &str) -> BTreeMap<String, FileTarget> {
        let mut files = self.files.clone();
        if let Some(per_os) = self.files_os.get(os) {
            for (template, target) in per_os {
                files.insert(template.clone(), target.clone());
            }
        }
        files
    }

    pub fn supports_os(&self, os: &str) -> bool {
        self.os.is_empty() || self.os.iter().any(|o| o == os)
    }

    /// Raw template source by name, from the embedded dir.
    pub fn template_source(&self, name: &str) -> Result<String> {
        // include_dir resolves relative to the root: "<module>/templates/<name>".
        let path = format!("{}/templates/{name}", self.name);
        let file = MODULES
            .get_file(&path)
            .ok_or_else(|| DotError::TemplateNotFound {
                module: self.name.clone(),
                template: name.to_string(),
            })?;
        file.contents_utf8().map(str::to_string).ok_or_else(|| {
            DotError::Other(anyhow::anyhow!(
                "template {name} in module {} is not valid UTF-8",
                self.name
            ))
        })
    }

    /// Overlay templates from `<config_dir>/modules/<name>/templates/` on
    /// top of the embedded ones. Returns (name, source) pairs for whichever
    /// templates the overlay provides.
    pub fn overlay_templates(&self, overlay_root: &std::path::Path) -> Vec<(String, String)> {
        let dir = overlay_root
            .join("modules")
            .join(&self.name)
            .join("templates");
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return out,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(src) = std::fs::read_to_string(&path) {
                        out.push((name.to_string(), src));
                    }
                }
            }
        }
        out
    }
}

fn is_os_name(key: &str) -> bool {
    matches!(key, "macos" | "linux" | "windows" | "freebsd" | "openbsd")
}

fn table_to_targets(value: toml::Value) -> Result<BTreeMap<String, FileTarget>> {
    let table = match value {
        toml::Value::Table(t) => t,
        v => {
            return Err(DotError::Other(anyhow::anyhow!(
                "expected a files table, got {v}"
            )))
        }
    };
    let mut out = BTreeMap::new();
    for (k, v) in table {
        let target: FileTarget = v
            .try_into()
            .map_err(|e| DotError::Other(anyhow::anyhow!("file target {k}: {e}")))?;
        out.insert(k, target);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_modules_parse() {
        let all = Module::all();
        assert!(!all.is_empty(), "no embedded modules found");
        for m in &all {
            assert_eq!(
                m.name,
                m.name.to_lowercase(),
                "{}: name not lowercase",
                m.name
            );
            assert!(
                !m.files.is_empty() || !m.files_os.is_empty(),
                "{}: no file targets",
                m.name
            );
            for template in m.files_for_os("macos").keys() {
                let src = m.template_source(template);
                assert!(src.is_ok(), "{}: missing template {template}", m.name);
            }
        }
    }

    #[test]
    fn parse_files_os() {
        let raw = r#"
name = "test"
description = "t"

[files]
"a.tmpl" = "~/x"

[files.linux]
"b.tmpl" = "~/y"
"#;
        let m = Module::parse(raw).unwrap();
        assert_eq!(m.files.len(), 1);
        assert!(m.files.contains_key("a.tmpl"));
        assert_eq!(m.files_os.get("linux").unwrap().len(), 1);
        let linux_files = m.files_for_os("linux");
        assert!(linux_files.contains_key("a.tmpl"));
        assert!(linux_files.contains_key("b.tmpl"));
    }

    #[test]
    fn parse_detailed_target() {
        let raw = r#"
name = "test"

[files]
"a.tmpl" = { path = "~/x", mode = 448 }
"#;
        let m = Module::parse(raw).unwrap();
        let t = m.files.get("a.tmpl").unwrap();
        assert_eq!(t.path(), "~/x");
        assert_eq!(t.mode(), 0o700);
    }

    #[test]
    fn os_support() {
        let m = Module {
            name: "x".into(),
            description: String::new(),
            os: vec!["macos".into()],
            defaults: toml::Value::Table(Default::default()),
            files: Default::default(),
            files_os: Default::default(),
            sensitive: vec![],
        };
        assert!(m.supports_os("macos"));
        assert!(!m.supports_os("linux"));
    }
}
