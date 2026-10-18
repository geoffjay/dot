//! `dot backup` and `dot restore`.

use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;

pub(crate) fn run_backup(module_filter: &[String], keep: usize) -> Result<()> {
    let (home, config, _facts, modules) = crate::load_context(module_filter)?;
    let manifest = dot_core::state::Manifest::load(&home)?;

    // If scoped to modules, snapshot only their files: filter the manifest
    // by recording a temporary view.
    let effective = if module_filter.is_empty() {
        manifest
    } else {
        let mut filtered = dot_core::state::Manifest::default();
        for target in manifest.targets(None) {
            let key = target.display().to_string();
            if let Some(entry) = manifest.files.get(&key) {
                if modules.iter().any(|m| m.name == entry.module) {
                    filtered.files.insert(key, entry.clone());
                }
            }
        }
        filtered
    };

    let count = effective.files.len();
    if count == 0 {
        println!("no managed files to back up; run `dot write` first");
        return Ok(());
    }

    let config_path = dot_core::Config::resolve_path(None, &home);
    let id = dot_core::backup::snapshot(
        &home,
        &effective,
        &config,
        &config_path,
        dot_core::backup::UtcNow::real(),
    )
    .map_err(anyhow::Error::from)?;

    let pruned = dot_core::backup::prune_backups(&home, keep).map_err(anyhow::Error::from)?;

    println!("backed up {count} files to {id}");
    if !pruned.is_empty() {
        println!("pruned {} old snapshot(s)", pruned.len());
    }
    Ok(())
}

pub(crate) fn run_restore(backup: Option<String>, force: bool, list: bool) -> Result<()> {
    let home = crate::home_dir()?;

    if list {
        let ids = dot_core::backup::list(&home);
        if ids.is_empty() {
            println!("no backups found");
        }
        for (idx, id) in ids.iter().enumerate() {
            let marker = if idx == 0 { " (latest)" } else { "" };
            println!("{id}{marker}");
        }
        return Ok(());
    }

    let restored =
        dot_core::backup::restore(&home, backup.as_deref(), force).map_err(anyhow::Error::from)?;

    if restored.is_empty() {
        println!("nothing to restore (empty backup)");
        return Ok(());
    }
    for target in &restored {
        println!("restored {}", target.display());
    }
    println!("\n{} file(s) restored", restored.len());
    Ok(())
}

/// Kept for API symmetry with other commands' module filtering; restore
/// works off snapshot content, not the live manifest.
#[allow(dead_code)]
fn restore_targets(_modules: &[String]) -> Vec<PathBuf> {
    Vec::new()
}

#[allow(dead_code)]
fn now_tag() -> String {
    Utc::now().format("%Y%m%d-%H%M%S").to_string()
}
