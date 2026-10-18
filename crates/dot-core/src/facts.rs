//! Host facts detected at write time.

use serde::Serialize;

/// Facts injected into every template as `host.*`.
#[derive(Debug, Clone, Serialize)]
pub struct HostFacts {
    pub os: String,
    pub distro: String,
    pub arch: String,
    pub hostname: String,
    pub username: String,
    pub package_managers: Vec<String>,
}

impl HostFacts {
    /// Names of package managers available on this host (used to gate
    /// install-channel logic in `dot update` and templates).
    pub fn has_pm(&self, pm: &str) -> bool {
        self.package_managers.iter().any(|p| p == pm)
    }
}

/// Detect host facts. `tools` is the list of binaries to probe for the
/// custom `has()` template function.
pub fn detect() -> HostFacts {
    let info = os_info::get();

    // std::env::consts::OS is "macos" | "linux" | ...; os_info's Type is
    // serialized as e.g. "Macos"/"Linux" — normalize to the lowercase form
    // used in module `os = [...]` lists.
    let os = std::env::consts::OS.to_string();
    let distro = info.os_type().to_string().to_lowercase();

    let hostname = hostname_str();
    let username = username_str();

    let package_managers = detect_package_managers();

    HostFacts {
        os,
        distro,
        arch: std::env::consts::ARCH.to_string(),
        hostname,
        username,
        package_managers,
    }
}

fn hostname_str() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok().filter(|s| !s.is_empty()))
        .or_else(scrape_hostname_cmd)
        .unwrap_or_else(|| "localhost".to_string())
}

#[cfg(not(target_os = "macos"))]
fn scrape_hostname_cmd() -> Option<String> {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(target_os = "macos")]
fn scrape_hostname_cmd() -> Option<String> {
    // `hostname` prints e.g. "worktop.local" on macOS; keep the short name
    // to match how hosts are usually keyed.
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .map(|s| s.split('.').next().unwrap_or(&s).to_string())
        .filter(|s| !s.is_empty())
}

fn username_str() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn detect_package_managers() -> Vec<String> {
    let candidates = ["brew", "pacman", "yay", "paru", "dnf", "apt", "apk", "nix"];
    candidates
        .iter()
        .filter(|c| which::which(c).is_ok())
        .map(|c| c.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_runs() {
        let f = detect();
        assert!(!f.os.is_empty());
        assert!(!f.arch.is_empty());
        assert!(!f.hostname.is_empty());
        assert!(!f.username.is_empty());
    }

    #[test]
    fn has_pm_matches() {
        let f = HostFacts {
            os: "macos".into(),
            distro: "macos".into(),
            arch: "aarch64".into(),
            hostname: "h".into(),
            username: "u".into(),
            package_managers: vec!["brew".into()],
        };
        assert!(f.has_pm("brew"));
        assert!(!f.has_pm("pacman"));
    }
}
