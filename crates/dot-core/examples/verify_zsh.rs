// Throwaway verification: render the zsh module against a temp HOME and
// diff every file against the live host state. Files listed in INTENDED
// are expected to differ (they were deliberately templated or guarded);
// everything else must be byte-identical.
fn main() -> anyhow::Result<()> {
    use std::path::Path;

    let home = Path::new("/Users/geoff");
    let tmp = std::env::temp_dir().join("dot-zsh-verify");
    std::fs::remove_dir_all(&tmp).ok();
    std::fs::create_dir_all(&tmp)?;

    // Config mirroring the host user + module defaults.
    let config = dot_core::Config {
        version: 1,
        user: dot_core::UserConfig {
            name: "Geoff".into(),
            email: "g@x.io".into(),
            theme: "nord".into(),
        },
        modules: Default::default(),
        hosts: Default::default(),
    };
    let facts = dot_core::facts::detect();
    let module = dot_core::Module::by_name("zsh")?;
    let rendered =
        dot_core::render::render_module(&module, &config, &facts, &tmp, None, &[], "0.1.0")?;

    // Fragments intentionally changed by templating/guards.
    let intended: &[&str] = &[
        ".zshrc",               // managed header + clio guard + hist templated
        ".zprofile",            // rewritten with guards
        ".zshrc.d/10-alias",    // bat-theme templated
        ".zshrc.d/10-prompt",   // starship guard
        ".zshrc.d/20-devenv",   // java/android templated
        ".zshrc.d/20-yarn",     // yarn guard
        ".zshrc.d/40-gcloud",   // gcloud-root templated
        ".zshrc.d/40-gitspice", // guard + header
    ];

    let mut identical = 0usize;
    let mut differing: Vec<String> = Vec::new();

    for f in &rendered {
        let rel = f
            .target
            .strip_prefix(&tmp)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let live = home.join(&rel);
        let live_content = std::fs::read_to_string(&live);
        match live_content {
            Ok(live_content) if live_content == f.content => identical += 1,
            Ok(live_content) => {
                let intended_diff = intended.contains(&rel.as_str());
                println!(
                    "{} {} ({} live {} rendered)",
                    if intended_diff {
                        "INTENDED"
                    } else {
                        "UNEXPECTED"
                    },
                    rel,
                    live_content.len(),
                    f.content.len()
                );
                if !intended_diff {
                    differing.push(rel);
                }
            }
            Err(_) => println!("MISSING live file: {rel}"),
        }
    }

    println!(
        "\n{identical} byte-identical, {} intended diffs",
        rendered.len() - identical - differing.len()
    );
    if !differing.is_empty() {
        println!("UNEXPECTED diffs: {differing:?}");
        std::process::exit(1);
    }
    Ok(())
}
