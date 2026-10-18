//! User configuration (`~/.config/dot/config.toml`).
//!
//! Precedence (low → high): module defaults → `[modules.<m>]` →
//! `[hosts.<hostname>.modules.<m>]` → CLI `--set` overrides.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{DotError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Schema version of the config file.
    #[serde(default)]
    pub version: u32,

    #[serde(default)]
    pub user: UserConfig,

    /// Per-module settings; absence of a module here means "default".
    #[serde(default)]
    pub modules: BTreeMap<String, toml::Value>,

    /// Per-host overrides, keyed by hostname.
    #[serde(default)]
    pub hosts: BTreeMap<String, HostConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub email: String,

    #[serde(default)]
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    #[serde(default)]
    pub modules: BTreeMap<String, toml::Value>,
}

impl Config {
    /// Resolve the config path: explicit override > env > XDG canonical >
    /// legacy `~/.dotrc`.
    pub fn resolve_path(explicit: Option<&Path>, home: &Path) -> PathBuf {
        if let Some(p) = explicit {
            return p.to_path_buf();
        }
        if let Ok(env) = std::env::var("DOT_CONFIG") {
            if !env.is_empty() {
                return PathBuf::from(env);
            }
        }
        let canonical = home.join(".config/dot/config.toml");
        if canonical.exists() {
            return canonical;
        }
        let legacy = home.join(".dotrc");
        if legacy.exists() {
            return legacy;
        }
        canonical
    }

    /// Load the config from `path`.
    pub fn load(path: &Path) -> Result<Config> {
        let raw = std::fs::read_to_string(path).map_err(|e| DotError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        toml::from_str::<Config>(&raw).map_err(|source| DotError::ConfigInvalid {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Serialize to a TOML string (used by `dot init`).
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| DotError::Other(e.into()))
    }

    /// Effective settings for a module: module defaults deep-merged with
    /// `[modules.<m>]`, then `[hosts.<hostname>.modules.<m>]`, then `cli`
    /// overrides — the last two keyed by hostname / free-form keys.
    pub fn module_settings(
        &self,
        module: &str,
        hostname: &str,
        cli_overrides: &[String],
    ) -> toml::Value {
        // Base: module defaults come from module.toml and are merged by the
        // caller (render); here we only compose the config layers.
        let mut out = toml::Value::Table(Default::default());

        // `[modules.<m>]` may be a bare bool (from init's selection output)
        // — that's an enable flag, not settings; only tables merge here.
        if let Some(toml::Value::Table(m)) = self.modules.get(module) {
            merge_toml(&mut out, &toml::Value::Table(m.clone()));
        }
        if let Some(host) = self.hosts.get(hostname) {
            if let Some(m) = host.modules.get(module) {
                merge_toml(&mut out, m);
            }
        }
        for kv in cli_overrides {
            if let Some((key, value)) = kv.split_once('=') {
                let prefix = format!("modules.{module}.");
                let field = key.strip_prefix(&prefix).map(str::to_string).or_else(|| {
                    key.rsplit_once('.').and_then(|(m, f)| {
                        if m == module {
                            Some(f.to_string())
                        } else {
                            None
                        }
                    })
                });
                let Some(field) = field else { continue };
                let parsed = parse_cli_value(value);
                let mut table = toml::Value::Table(Default::default());
                insert_path(&mut table, &field, parsed);
                merge_toml(&mut out, &table);
            }
        }
        out
    }

    /// Is `module` enabled? Enabled unless explicitly disabled (opt-out).
    pub fn module_enabled(&self, module: &str, hostname: &str) -> bool {
        // Both forms are valid: `example = false` (bare bool, written by
        // init) or `[modules.example] enabled = false`.
        let enabled_from = |v: Option<&toml::Value>| -> Option<bool> {
            match v {
                Some(toml::Value::Boolean(b)) => Some(*b),
                Some(toml::Value::Table(t)) => t.get("enabled").and_then(|e| e.as_bool()),
                _ => None,
            }
        };

        let mut enabled = enabled_from(self.modules.get(module)).unwrap_or(true);
        if let Some(host) = self.hosts.get(hostname) {
            if let Some(v) = enabled_from(host.modules.get(module)) {
                enabled = v;
            }
        }
        enabled
    }
}

/// Deep-merge `src` into `dst`: tables merge recursively, everything else
/// replaces.
pub fn merge_toml(dst: &mut toml::Value, src: &toml::Value) {
    match (dst, src) {
        (toml::Value::Table(d), toml::Value::Table(s)) => {
            for (k, v) in s {
                match d.get_mut(k) {
                    Some(existing) => merge_toml(existing, v),
                    None => {
                        d.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (dst, src) => *dst = src.clone(),
    }
}

fn insert_path(table: &mut toml::Value, dotted: &str, value: toml::Value) {
    let mut parts: Vec<&str> = dotted.split('.').collect();
    let last = parts.pop().expect("split yields at least one part");
    let mut cur = table;
    for part in parts {
        let entry = cur
            .as_table_mut()
            .expect("insert_path walks tables")
            .entry(part.to_string())
            .or_insert_with(|| toml::Value::Table(Default::default()));
        if !entry.is_table() {
            *entry = toml::Value::Table(Default::default());
        }
        cur = entry;
    }
    cur.as_table_mut()
        .expect("insert_path walks tables")
        .insert(last.to_string(), value);
}

/// Parse a CLI `--set` value: bare bool/int/float stay typed, `a,b,c`
/// becomes an array, everything else is a string.
fn parse_cli_value(raw: &str) -> toml::Value {
    match raw {
        "true" => toml::Value::Boolean(true),
        "false" => toml::Value::Boolean(false),
        _ => {
            if let Ok(i) = raw.parse::<i64>() {
                toml::Value::Integer(i)
            } else if let Ok(f) = raw.parse::<f64>() {
                toml::Value::Float(f)
            } else if raw.contains(',') {
                toml::Value::Array(
                    raw.split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(|s| toml::Value::String(s.to_string()))
                        .collect(),
                )
            } else {
                toml::Value::String(raw.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(raw: &str) -> Config {
        toml::from_str(raw).unwrap()
    }

    #[test]
    fn enabled_by_default() {
        let c = cfg("");
        assert!(c.module_enabled("zsh", "any"));
    }

    #[test]
    fn disabled_module_is_opt_out() {
        let c = cfg("[modules.zsh]\nenabled = false");
        assert!(!c.module_enabled("zsh", "any"));
    }

    #[test]
    fn host_overrides_module() {
        let c = cfg(r#"
[modules.zsh]
enabled = false

[hosts.worktop.modules.zsh]
enabled = true
"#);
        assert!(!c.module_enabled("zsh", "laptop"));
        assert!(c.module_enabled("zsh", "worktop"));
    }

    #[test]
    fn settings_merge_deep() {
        let c = cfg(r#"
[modules.zsh]
plugins = ["a", "b"]
prompt = "starship"

[hosts.worktop.modules.zsh]
plugins = ["c"]
"#);
        let s = c.module_settings("zsh", "worktop", &[]);
        assert_eq!(
            s.get("plugins").unwrap(),
            &toml::Value::Array(vec![toml::Value::String("c".into())])
        );
        // non-overlapping key survives the merge
        assert_eq!(
            s.get("prompt").unwrap(),
            &toml::Value::String("starship".into())
        );
    }

    #[test]
    fn cli_overrides_win() {
        let c = cfg("[modules.zsh]\nprompt = \"starship\"");
        let s = c.module_settings("zsh", "any", &["zsh.prompt=pure".to_string()]);
        assert_eq!(
            s.get("prompt").unwrap(),
            &toml::Value::String("pure".into())
        );
    }

    #[test]
    fn cli_comma_value_is_array() {
        let c = cfg("");
        let s = c.module_settings("zsh", "any", &["zsh.plugins=a,b".to_string()]);
        assert_eq!(
            s.get("plugins").unwrap(),
            &toml::Value::Array(vec![
                toml::Value::String("a".into()),
                toml::Value::String("b".into()),
            ])
        );
    }

    #[test]
    fn toml_roundtrip() {
        let c = cfg(r#"
version = 1

[user]
name = "Geoff"
email = "g@example.com"
theme = "nord"

[modules.zsh]
enabled = true

[hosts.worktop.modules.zsh]
plugins = ["x"]
"#);
        let out = c.to_toml().unwrap();
        let back: Config = toml::from_str(&out).unwrap();
        assert_eq!(back.user.name, "Geoff");
        assert!(back.hosts.contains_key("worktop"));
    }
}
