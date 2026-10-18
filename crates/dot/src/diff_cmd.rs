//! `dot diff`: preview what `dot write` would change.

use anyhow::Result;

use dot_core::plan::WriteAction;

pub(crate) fn run(module_filter: &[String]) -> Result<()> {
    let (home, config, facts, modules) = crate::load_context(module_filter)?;
    let manifest = dot_core::state::Manifest::load(&home)?;

    let overlay_root = home.join(".config/dot");
    let version = env!("CARGO_PKG_VERSION");

    let mut rendered_all = Vec::new();
    for module in &modules {
        rendered_all.extend(dot_core::render::render_module(
            module,
            &config,
            &facts,
            &home,
            Some(&overlay_root),
            &[],
            version,
        )?);
    }

    let managed = |p: &std::path::Path| manifest.contains(p);
    let plan = dot_core::plan::plan_write(rendered_all, &managed)?;

    let mut differences = 0;
    for planned in &plan.files {
        let r = &planned.rendered;
        match planned.action {
            WriteAction::Unchanged => continue,
            WriteAction::Create => {
                differences += 1;
                let diff = dot_core::diff::unified_hunks(&r.target, None, &r.content);
                print!("{diff}");
            }
            WriteAction::Update => {
                differences += 1;
                let existing = std::fs::read_to_string(&r.target).unwrap_or_default();
                let diff = dot_core::diff::unified_hunks(&r.target, Some(&existing), &r.content);
                print!("{diff}");
                if planned.adopting {
                    println!(
                        "# note: {} is not managed yet; dot write would adopt it (original backed up)",
                        r.target.display()
                    );
                }
            }
        }
    }

    println!();
    let (create, update, unchanged) = plan.counts();
    println!("{create} to create, {update} to update, {unchanged} unchanged");

    // diff(1) exit codes: 1 = differences, 0 = identical. Errors exit 2
    // via main's error path.
    if differences > 0 {
        std::process::exit(1);
    }
    Ok(())
}
