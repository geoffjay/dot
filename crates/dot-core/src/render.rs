//! Template rendering with minijinja.
//!
//! Every template gets `user.*`, `host.*`, `module`, `dot_version`, and a
//! `has("tool")` function that probes PATH at write time.

use std::path::{Path, PathBuf};

use minijinja::{Environment, Value};

use crate::config::Config;
use crate::error::{DotError, Result};
use crate::facts::HostFacts;
use crate::module::Module;

#[derive(Debug, Clone)]
pub struct RenderedFile {
    /// Name of the module this file belongs to.
    pub module: String,

    /// Resolved absolute target path.
    pub target: PathBuf,

    /// Template name it was rendered from.
    pub template: String,

    /// Rendered content.
    pub content: String,

    /// File mode to apply (unix).
    pub mode: u32,
}

/// Render all files of `module` for this host.
///
/// `home` is `$HOME`; every target is resolved against it and must stay
/// inside it. `overlay_root` (optional, usually the dot config dir)
/// supplies user template overrides; the effective source for a template is
/// overlay-if-present, embedded otherwise.
pub fn render_module(
    module: &Module,
    config: &Config,
    facts: &HostFacts,
    home: &Path,
    overlay_root: Option<&Path>,
    cli_overrides: &[String],
    dot_version: &str,
) -> Result<Vec<RenderedFile>> {
    if !module.supports_os(&facts.os) {
        return Err(DotError::UnsupportedOs {
            module: module.name.clone(),
            os: facts.os.clone(),
        });
    }

    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    let mut context = serde_json::Map::new();
    context.insert(
        "user".to_string(),
        serde_json::to_value(&config.user).unwrap_or_default(),
    );
    context.insert(
        "host".to_string(),
        serde_json::to_value(facts).unwrap_or_default(),
    );
    context.insert(
        "module".to_string(),
        serde_json::Value::String(module.name.clone()),
    );
    context.insert(
        "dot_version".to_string(),
        serde_json::Value::String(dot_version.to_string()),
    );

    let settings = {
        let mut s = module.defaults.clone();
        let layered = config.module_settings(&module.name, &facts.hostname, cli_overrides);
        crate::config::merge_toml(&mut s, &layered);
        s
    };
    context.insert("settings".to_string(), toml_to_json(&settings));

    // `has("tool")`: PATH lookup at write time. Mutex (not RefCell) because
    // minijinja functions must be Send + Sync.
    let tools_cache: std::sync::Mutex<std::collections::BTreeMap<String, bool>> =
        std::sync::Mutex::new(Default::default());
    env.add_function("has", move |name: String| {
        let Ok(mut cache) = tools_cache.lock() else {
            return false;
        };
        *cache
            .entry(name.clone())
            .or_insert_with(|| which::which(&name).is_ok())
    });

    // Collect template sources into owned strings first: minijinja's
    // `add_template` borrows for the lifetime of the environment, so the
    // sources must outlive `env` — keep them in `owned` below.
    let mut owned: Vec<(String, String)> = Vec::new();
    if let Some(root) = overlay_root {
        owned.extend(module.overlay_templates(root));
    }
    let files = module.files_for_os(&facts.os);
    for template in files.keys() {
        if !owned.iter().any(|(n, _)| n == template) {
            let source = module.template_source(template)?;
            owned.push((template.clone(), source));
        }
    }

    for (name, source) in &owned {
        env.add_template(name, source)
            .map_err(|e| DotError::RenderError {
                module: module.name.clone(),
                template: name.clone(),
                source: e,
            })?;
    }

    let mut rendered = Vec::with_capacity(files.len());
    for (template, target) in &files {
        let tmpl = env
            .get_template(template)
            .map_err(|e| DotError::RenderError {
                module: module.name.clone(),
                template: template.clone(),
                source: e,
            })?;
        let content =
            tmpl.render(Value::from_serialize(&context))
                .map_err(|e| DotError::RenderError {
                    module: module.name.clone(),
                    template: template.clone(),
                    source: e,
                })?;

        let resolved = resolve_target(target.path(), home)?;
        rendered.push(RenderedFile {
            module: module.name.clone(),
            target: resolved,
            template: template.clone(),
            content,
            mode: target.mode(),
        });
    }

    Ok(rendered)
}

/// Resolve a `~`-relative module target against `home`, enforcing the
/// HOME-only invariant.
pub fn resolve_target(path: &str, home: &Path) -> Result<PathBuf> {
    let expanded = if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else if path == "~" {
        home.to_path_buf()
    } else if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        return Err(DotError::TargetOutsideHome(PathBuf::from(path)));
    };

    // Absolute paths must live under home. Comparison is lexical after
    // normalization; symlinked HOMEs (e.g. macOS /var) are accepted by
    // comparing components.
    let normalized = normalize(&expanded);
    if !normalized.starts_with(home) {
        return Err(DotError::TargetOutsideHome(normalized));
    }
    Ok(normalized)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn toml_to_json(value: &toml::Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UserConfig;

    #[test]
    fn resolve_target_rejects_outside_home() {
        let home = Path::new("/home/u");
        assert!(resolve_target("~/x", home).is_ok());
        assert!(resolve_target("/etc/passwd", home).is_err());
        assert!(resolve_target("~/../../etc/passwd", home).is_err());
        assert!(resolve_target("relative/x", home).is_err());
    }

    fn facts() -> HostFacts {
        HostFacts {
            os: "macos".into(),
            distro: "macos".into(),
            arch: "aarch64".into(),
            hostname: "testhost".into(),
            username: "tester".into(),
            package_managers: vec!["brew".into()],
        }
    }

    #[test]
    fn render_embedded_module() {
        let module = Module::by_name("example").expect("example module embedded");
        let config = Config {
            version: 1,
            user: UserConfig {
                name: "Tester".into(),
                email: "t@example.com".into(),
                theme: "nord".into(),
            },
            modules: Default::default(),
            hosts: Default::default(),
        };
        let rendered = render_module(
            &module,
            &config,
            &facts(),
            Path::new("/home/u"),
            None,
            &[],
            "0.1.0",
        )
        .unwrap();
        assert!(!rendered.is_empty());
        for f in &rendered {
            assert!(f.target.starts_with("/home/u"));
        }
    }
}
