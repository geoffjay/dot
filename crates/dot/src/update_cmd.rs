//! `dot update`: upgrade dot via its install channel.

use anyhow::{bail, Context, Result};

const REPO: &str = "geoffjay/dot";
const AUR_PACKAGE: &str = "dot-bin";

#[derive(Debug, PartialEq)]
enum Channel {
    Homebrew,
    Aur,
    Manual,
}

pub(crate) fn run(check: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

    if check {
        return check_latest(current);
    }

    match detect_channel() {
        Channel::Homebrew => {
            let output = std::process::Command::new("brew")
                .args(["list", "--versions", "dot"])
                .output()
                .context("running brew list")?;
            if !output.status.success() {
                bail!("brew is installed but dot is not managed by it");
            }
            let installed = String::from_utf8_lossy(&output.stdout);
            println!("updating via Homebrew (currently: {installed})");
            let status = std::process::Command::new("brew")
                .args(["upgrade", "dot"])
                .status()
                .context("running brew upgrade dot")?;
            if !status.success() {
                bail!("brew upgrade dot failed");
            }
            println!("updated");
            Ok(())
        }
        Channel::Aur => {
            let helper = ["yay", "paru"]
                .into_iter()
                .find(|h| which::which(h).is_ok())
                .context("no AUR helper found (install yay or paru)")?;
            println!("updating {AUR_PACKAGE} via {helper}");
            let status = std::process::Command::new(helper)
                .args(["-S", "--needed", AUR_PACKAGE])
                .status()
                .with_context(|| format!("running {helper} -S {AUR_PACKAGE}"))?;
            if !status.success() {
                bail!("{helper} -S {AUR_PACKAGE} failed");
            }
            println!("updated");
            Ok(())
        }
        Channel::Manual => {
            println!("dot was not installed via brew or an AUR helper.");
            print_manual_instructions();
            Ok(())
        }
    }
}

fn detect_channel() -> Channel {
    let on_macos = cfg!(target_os = "macos");
    let brew_installed = which::which("brew").is_ok();
    let pacman_owned = pacman_owns_dot();

    if brew_installed && (on_macos || brew_manages_dot()) {
        Channel::Homebrew
    } else if pacman_owned {
        Channel::Aur
    } else if brew_installed {
        Channel::Homebrew
    } else {
        Channel::Manual
    }
}

fn brew_manages_dot() -> bool {
    std::process::Command::new("brew")
        .args(["list", "--versions", "dot"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn pacman_owns_dot() -> bool {
    std::process::Command::new("pacman")
        .args(["-Qo", "/usr/bin/dot"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn check_latest(current: &str) -> Result<()> {
    let latest = latest_release_tag().context("querying GitHub for the latest release")?;
    let latest_version = latest.trim_start_matches('v');
    println!("current: {current}");
    println!("latest:  {latest_version}");

    if latest_version == current {
        println!("up to date");
        Ok(())
    } else {
        println!("update available via your install channel; run `dot update`");
        Ok(())
    }
}

fn latest_release_tag() -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("dot/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitHub API returned {status} for {url}");
    }
    let json: serde_json::Value = response.json()?;
    json.get("tag_name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .context("release payload had no tag_name")
}

fn print_manual_instructions() {
    println!(
        "manual update from a release tarball:

  curl -fsSL https://github.com/{REPO}/releases/latest -o /dev/null \\
    && gh release download --repo {REPO} --pattern 'dot-*' --output /tmp/dot
  # verify against checksums.txt, then:
  tar -xzf dot-<target>.tar.gz && ./dot --version"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_detection_runs() {
        // smoke: never panics regardless of environment
        let channel = detect_channel();
        assert!(matches!(
            channel,
            Channel::Homebrew | Channel::Aur | Channel::Manual
        ));
    }
}
