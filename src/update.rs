use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub const RELEASES_PAGE: &str = "https://github.com/Chakyiu/small-window-translator/releases";
const LATEST_URL: &str =
    "https://api.github.com/repos/Chakyiu/small-window-translator/releases/latest";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    UpToDate {
        current: String,
        latest: String,
    },
    UpdateAvailable {
        current: String,
        latest: String,
        url: String,
    },
    NoRelease {
        current: String,
    },
}

impl CheckOutcome {
    pub fn message(&self) -> String {
        match self {
            Self::UpToDate { current, latest } if current == latest => {
                format!("You're on the latest version (v{current})")
            }
            Self::UpToDate { current, latest } => {
                format!("You're on v{current}; latest release is v{latest}")
            }
            Self::UpdateAvailable {
                current, latest, ..
            } => format!("Version {latest} is available (you have {current})"),
            Self::NoRelease { current } => {
                format!("No GitHub release yet. You're on v{current}")
            }
        }
    }

    pub fn open_url(&self) -> Option<&str> {
        match self {
            Self::UpdateAvailable { url, .. } => Some(url),
            Self::NoRelease { .. } => Some(RELEASES_PAGE),
            Self::UpToDate { .. } => None,
        }
    }

    pub fn open_label(&self) -> Option<&'static str> {
        match self {
            Self::UpdateAvailable { .. } => Some("Open download page"),
            Self::NoRelease { .. } => Some("Open GitHub releases"),
            Self::UpToDate { .. } => None,
        }
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub fn check() -> Result<CheckOutcome> {
    let current = current_version().to_string();
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!(
            "swtrans/{current} (+https://github.com/Chakyiu/small-window-translator)"
        ))
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let response = client
        .get(LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("Could not reach GitHub")?;
    let status = response.status();
    if status.as_u16() == 404 {
        return Ok(CheckOutcome::NoRelease { current });
    }
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        bail!("GitHub HTTP {status}: {body}");
    }
    let release: GithubRelease = serde_json::from_str(&body).context("GitHub release JSON")?;
    if release.draft || release.prerelease {
        return Ok(CheckOutcome::NoRelease { current });
    }
    let latest = normalize_version(&release.tag_name);
    if version_newer(&latest, &current) {
        Ok(CheckOutcome::UpdateAvailable {
            current,
            latest,
            url: release.html_url,
        })
    } else {
        Ok(CheckOutcome::UpToDate { current, latest })
    }
}

fn normalize_version(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).to_string()
}

fn version_parts(ver: &str) -> Vec<u64> {
    let normalized = normalize_version(ver);
    let core = match normalized.split_once(['-', '+']) {
        Some((core, _)) => core,
        None => normalized.as_str(),
    };
    core.split('.').filter_map(|p| p.parse().ok()).collect()
}

fn version_newer(latest: &str, current: &str) -> bool {
    let mut a = version_parts(latest);
    let mut b = version_parts(current);
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_newer_release() {
        assert!(version_newer("0.1.1", "0.1.0"));
        assert!(version_newer("v0.2.0", "0.1.9"));
        assert!(!version_newer("0.1.0", "0.1.0"));
        assert!(!version_newer("0.1.0", "0.1.1"));
        assert!(!version_newer("0.1.0-beta", "0.1.0"));
    }
}
