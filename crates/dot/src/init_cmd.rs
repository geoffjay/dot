//! `dot init`: interactive config creation.

use std::io::Write;

use anyhow::{bail, Context, Result};
use dialoguer::{Confirm, Input, MultiSelect};

use dot_core::config::{Config, UserConfig};

/// Answer sheet for --non-interactive (subset of the config schema).
#[derive(serde::Deserialize, Default)]
struct Answers {
    #[serde(default)]
    user: UserAnswers,
    #[serde(default)]
    modules: Vec<String>,
}

#[derive(serde::Deserialize, Default)]
struct UserAnswers {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    theme: Option<String>,
}

pub(crate) fn run(force: bool, non_interactive: bool, answers: Option<String>) -> Result<()> {
    let home = crate::home_dir()?;
    let path = Config::resolve_path(None, &home);

    if path.exists() && !force {
        bail!(
            "config already exists at {}; use --force to overwrite",
            path.display()
        );
    }

    let available = dot_core::Module::all();

    let answers: Answers = if non_interactive {
        let file = answers
            .as_deref()
            .context("--non-interactive requires --answers <file>")?;
        let raw = std::fs::read_to_string(file)
            .with_context(|| format!("reading answers file {}", file))?;
        toml::from_str(&raw).with_context(|| format!("parsing answers file {}", file))?
    } else {
        prompt(&available)?
    };

    let config = build_config(answers, &available);
    let toml_out = config.to_toml().map_err(anyhow::Error::from)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, toml_out).with_context(|| format!("writing {}", path.display()))?;

    println!("wrote {}", path.display());
    println!("next: `dot diff` to preview, `dot write` to install");
    Ok(())
}

fn prompt(available: &[dot_core::Module]) -> Result<Answers> {
    let mut stdout = std::io::stdout().lock();
    writeln!(
        stdout,
        "dot init — answer a few questions to generate ~/.config/dot/config.toml"
    )
    .ok();

    let name: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Your name")
        .interact_text()
        .context("prompt: name")?;

    let email: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Your email")
        .validate_with(|v: &String| -> Result<(), &str> {
            if v.is_empty() {
                Err("email must not be empty")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .context("prompt: email")?;

    let theme: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Preferred theme")
        .default("nord".into())
        .interact_text()
        .context("prompt: theme")?;

    let descriptions: Vec<String> = available
        .iter()
        .map(|m| format!("{} — {}", m.name, m.description))
        .collect();
    let selected = MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Enable modules (space to toggle)")
        .items(&descriptions)
        .defaults(&available.iter().map(|_| true).collect::<Vec<_>>())
        .interact()
        .context("prompt: modules")?;

    let modules: Vec<String> = selected
        .into_iter()
        .map(|i| available[i].name.clone())
        .collect();

    let proceed = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Write the config now?")
        .default(true)
        .interact()
        .context("prompt: confirm")?;
    if !proceed {
        bail!("aborted");
    }

    Ok(Answers {
        user: UserAnswers {
            name: Some(name),
            email: Some(email),
            theme: Some(theme),
        },
        modules,
    })
}

fn build_config(answers: Answers, available: &[dot_core::Module]) -> Config {
    let known: std::collections::HashSet<&str> =
        available.iter().map(|m| m.name.as_str()).collect();

    let mut config = Config {
        version: 1,
        user: UserConfig {
            name: answers.user.name.unwrap_or_default(),
            email: answers.user.email.unwrap_or_default(),
            theme: answers.user.theme.unwrap_or_else(|| "nord".into()),
        },
        modules: Default::default(),
        hosts: Default::default(),
    };

    for module in available {
        let enabled = answers.modules.iter().any(|m| m == &module.name);
        // Table form (`[modules.<name>] enabled = bool`) so settings can
        // be added under the same table later without a structural
        // conflict with a bare bool.
        let mut table = toml::Table::new();
        table.insert("enabled".to_string(), toml::Value::Boolean(enabled));
        config
            .modules
            .insert(module.name.clone(), toml::Value::Table(table));
    }
    // answers may reference modules unknown to this binary; ignore them
    // rather than writing config that would fail to load later
    for requested in answers.modules {
        if !known.contains(requested.as_str()) {
            eprintln!("dot: ignoring unknown module: {requested}");
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_config_marks_enabled() {
        let available = dot_core::Module::all();
        assert!(!available.is_empty());
        let first = available[0].name.clone();
        let cfg = build_config(
            Answers {
                user: UserAnswers {
                    name: Some("G".into()),
                    email: Some("g@x.com".into()),
                    theme: None,
                },
                modules: vec![first.clone()],
            },
            &available,
        );
        assert!(cfg.module_enabled(&first, "h"));
        assert_eq!(cfg.user.theme, "nord");
        // every available module has an explicit entry
        assert_eq!(cfg.modules.len(), available.len());
    }
}
