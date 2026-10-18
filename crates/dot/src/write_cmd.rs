//! `dot write`: render and install configuration.

use anyhow::Result;
use chrono::Utc;

pub(crate) fn run(module_filter: &[String], force: bool) -> Result<()> {
    let (home, config, facts, modules) = crate::load_context(module_filter)?;
    let mut manifest = dot_core::state::Manifest::load(&home)?;

    let overlay_root = home.join(".config/dot");
    let version = env!("CARGO_PKG_VERSION");

    // Render everything first: a render error must abort before any
    // filesystem mutation (plan §6: all-or-nothing).
    let mut rendered_all = Vec::new();
    for module in &modules {
        let files = dot_core::render::render_module(
            module,
            &config,
            &facts,
            &home,
            Some(&overlay_root),
            &[],
            version,
        )?;
        rendered_all.extend(files);
    }

    let managed = |p: &std::path::Path| manifest.contains(p);
    let plan = dot_core::plan::plan_write(rendered_all, &managed)?;

    let home_for_closure = home.clone();
    dot_core::plan::execute_plan(&plan, force, &move |r| {
        let original =
            std::fs::read(&r.target).map_err(|e| dot_core::DotError::io(&r.target, e))?;
        dot_core::backup::save_adoption(&home_for_closure, &r.target, &original)
    })
    .map_err(anyhow::Error::from)?;

    let now = Utc::now().to_rfc3339();
    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    for planned in &plan.files {
        let r = &planned.rendered;
        manifest.record(&r.target, &r.module, &r.template, &r.content, version, &now);
        match planned.action {
            dot_core::plan::WriteAction::Create => {
                created += 1;
                println!("create  {}", r.target.display());
            }
            dot_core::plan::WriteAction::Update => {
                updated += 1;
                println!("update  {}", r.target.display());
                if planned.adopting {
                    println!("        adopted (original saved to backups/pre-write)");
                }
            }
            dot_core::plan::WriteAction::Unchanged => {
                unchanged += 1;
            }
        }
    }

    manifest.save(&home).map_err(anyhow::Error::from)?;

    println!();
    println!("{created} created, {updated} updated, {unchanged} unchanged");
    Ok(())
}
