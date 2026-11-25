use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const RELEASES_PAGE: &str = "https://github.com/Chakyiu/small-window-translator/releases";
const LATEST_URL: &str =
    "https://api.github.com/repos/Chakyiu/small-window-translator/releases/latest";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMode {
    Nsis,
    MacAppZip,
    AppImageReplace,
    DebianPackage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallableUpdate {
    pub download_url: String,
    pub file_name: String,
    pub mode: InstallMode,
}

impl InstallableUpdate {
    /// Whether this build looks like a packaged install that can be updated in place.
    pub fn supported_in_this_process(&self) -> bool {
        if !packaged_install_context() {
            return false;
        }
        match self.mode {
            InstallMode::AppImageReplace => std::env::var("APPIMAGE").is_ok(),
            InstallMode::DebianPackage => {
                std::env::var("APPIMAGE").is_err() && linux_system_binary()
            }
            InstallMode::MacAppZip => macos_app_bundle_path().is_some(),
            InstallMode::Nsis => true,
        }
    }
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
        installable: Option<InstallableUpdate>,
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
                current,
                latest,
                installable,
                ..
            } => {
                let base =
                    format!("Version {latest} is available (you have {current})");
                if installable
                    .as_ref()
                    .is_some_and(|i| i.supported_in_this_process())
                {
                    format!("{base}. You can install it from here.")
                } else {
                    base
                }
            }
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
            Self::UpdateAvailable {
                installable: Some(i),
                ..
            } if i.supported_in_this_process() => Some("View release notes"),
            Self::UpdateAvailable { .. } => Some("Open download page"),
            Self::NoRelease { .. } => Some("Open GitHub releases"),
            Self::UpToDate { .. } => None,
        }
    }

    pub fn installable(&self) -> Option<&InstallableUpdate> {
        match self {
            Self::UpdateAvailable { installable, .. } => {
                installable
                    .as_ref()
                    .filter(|i| i.supported_in_this_process())
            }
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn check() -> Result<CheckOutcome> {
    let current = current_version().to_string();
    let client = http_client(&current)?;
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
    let installable = pick_asset(&release.assets);
    if version_newer(&latest, &current) {
        Ok(CheckOutcome::UpdateAvailable {
            current,
            latest,
            url: release.html_url,
            installable,
        })
    } else {
        Ok(CheckOutcome::UpToDate { current, latest })
    }
}

/// Download the release asset and start the platform-specific install/restart flow.
pub fn download_and_apply(update: &InstallableUpdate) -> Result<()> {
    let client = http_client(current_version())?;
    let tmp = std::env::temp_dir().join("swtrans-update");
    std::fs::create_dir_all(&tmp).context("Could not create update temp dir")?;
    let dest = tmp.join(&update.file_name);
    if dest.exists() {
        std::fs::remove_file(&dest).ok();
    }
    let mut response = client
        .get(&update.download_url)
        .send()
        .context("Could not download update")?;
    if !response.status().is_success() {
        bail!("Download HTTP {}", response.status());
    }
    let mut file = std::fs::File::create(&dest).context("Could not write update file")?;
    std::io::copy(&mut response, &mut file).context("Update download interrupted")?;
    drop(file);

    match update.mode {
        InstallMode::Nsis => apply_nsis(&dest),
        InstallMode::MacAppZip => apply_mac_zip(&dest, &tmp),
        InstallMode::AppImageReplace => apply_appimage(&dest),
        InstallMode::DebianPackage => apply_deb(&dest),
    }
}

fn http_client(user_agent_version: &str) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(format!(
            "swtrans/{user_agent_version} (+https://github.com/Chakyiu/small-window-translator)"
        ))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("HTTP client")
}

fn packaged_install_context() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let path = exe.to_string_lossy();
    #[cfg(debug_assertions)]
    {
        let _ = path;
        return false;
    }
    #[cfg(not(debug_assertions))]
    {
        !path.contains("/target/debug")
            && !path.contains("\\target\\debug")
            && !path.contains("/target/relwithdebinfo")
            && !path.contains("\\target\\relwithdebinfo")
    }
}

#[cfg(target_os = "linux")]
fn linux_system_binary() -> bool {
    std::env::var("APPIMAGE").is_err()
        && std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .is_some_and(|parent| {
                parent == Path::new("/usr/bin") || parent == Path::new("/usr/local/bin")
            })
}

#[cfg(not(target_os = "linux"))]
fn linux_system_binary() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn macos_app_bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos = exe.parent()?;
    if macos.file_name()?.to_str()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()?.to_str()? != "Contents" {
        return None;
    }
    let app = contents.parent()?;
    (app.extension()?.to_str()? == "app").then(|| app.to_path_buf())
}

#[cfg(not(target_os = "macos"))]
fn macos_app_bundle_path() -> Option<PathBuf> {
    None
}

fn pick_asset(assets: &[GithubAsset]) -> Option<InstallableUpdate> {
    let pick = |pred: &dyn Fn(&str) -> bool| -> Option<InstallableUpdate> {
        assets.iter().find(|a| pred(&a.name)).map(|a| InstallableUpdate {
            download_url: a.browser_download_url.clone(),
            file_name: a.name.clone(),
            mode: install_mode_for_asset(&a.name),
        })
    };

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return pick(&|name| name.ends_with("_x64-setup.exe") || name.ends_with("-x64-setup.exe"));

    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return pick(&|name| name.ends_with("_arm64-setup.exe") || name.ends_with("-arm64-setup.exe"));

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return pick(&|name| name.ends_with("macos-aarch64.zip"));

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return pick(&|name| name.ends_with("macos-x86_64.zip"));

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        if std::env::var("APPIMAGE").is_ok() {
            return pick(&|name| name.ends_with("_x86_64.AppImage"));
        }
        if linux_system_binary() {
            return pick(&|name| name.ends_with("_amd64.deb"));
        }
        return None;
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        if std::env::var("APPIMAGE").is_ok() {
            return pick(&|name| name.ends_with("_aarch64.AppImage"));
        }
        if linux_system_binary() {
            return pick(&|name| name.ends_with("_arm64.deb"));
        }
        return None;
    }

    #[cfg(target_os = "linux")]
    return None;

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return None;
}

fn install_mode_for_asset(name: &str) -> InstallMode {
    if name.ends_with("-setup.exe") || name.contains("-setup.exe") {
        InstallMode::Nsis
    } else if name.ends_with(".zip") && name.contains("macos") {
        InstallMode::MacAppZip
    } else if name.ends_with(".AppImage") {
        InstallMode::AppImageReplace
    } else if name.ends_with(".deb") {
        InstallMode::DebianPackage
    } else {
        InstallMode::Nsis
    }
}

#[cfg(windows)]
fn apply_nsis(installer: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    std::process::Command::new(installer)
        .arg("/S")
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .context("Could not start installer")?;
    Ok(())
}

#[cfg(not(windows))]
fn apply_nsis(_installer: &Path) -> Result<()> {
    bail!("NSIS install is only supported on Windows");
}

#[cfg(target_os = "macos")]
fn apply_mac_zip(archive: &Path, work_dir: &Path) -> Result<()> {
    let bundle_dest = macos_app_bundle_path().context("Not running from an .app bundle")?;
    let extract = work_dir.join("extracted");
    if extract.exists() {
        std::fs::remove_dir_all(&extract).ok();
    }
    std::fs::create_dir_all(&extract)?;
    let new_bundle = unzip_app_bundle(archive, &extract)?;
    let pid = std::process::id();
    let script = work_dir.join("swtrans-apply-update.sh");
    let script_body = format!(
        r#"#!/bin/bash
set -e
TARGET={target:?}
SOURCE={source:?}
PID={pid}
while kill -0 "$PID" 2>/dev/null; do sleep 0.3; done
sleep 1
rm -rf "$TARGET"
ditto "$SOURCE" "$TARGET"
open "$TARGET"
rm -rf {work:?}
"#,
        target = bundle_dest.display(),
        source = new_bundle.display(),
        work = work_dir.display(),
    );
    std::fs::write(&script, script_body)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    std::process::Command::new(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Could not start macOS update helper")?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn apply_mac_zip(_archive: &Path, _work_dir: &Path) -> Result<()> {
    bail!("macOS zip updates require macOS");
}

#[cfg(target_os = "linux")]
fn apply_appimage(downloaded: &Path) -> Result<()> {
    let current = std::env::var("APPIMAGE").context("APPIMAGE not set")?;
    let current = PathBuf::from(current);
    let staged = PathBuf::from(format!("{}.new", current.display()));
    if std::fs::rename(downloaded, &staged).is_err() {
        std::fs::copy(downloaded, &staged).context("Stage AppImage update")?;
        std::fs::remove_file(downloaded).ok();
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    let pid = std::process::id();
    let script = std::env::temp_dir().join("swtrans-apply-appimage.sh");
    let script_body = format!(
        r#"#!/bin/bash
set -e
TARGET={target:?}
NEW={new:?}
PID={pid}
while kill -0 "$PID" 2>/dev/null; do sleep 0.3; done
mv "$NEW" "$TARGET"
chmod +x "$TARGET"
exec "$TARGET" --hidden
"#,
        target = current.display(),
        new = staged.display(),
    );
    std::fs::write(&script, script_body)?;
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    std::process::Command::new(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Could not start AppImage update helper")?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn apply_appimage(_downloaded: &Path) -> Result<()> {
    bail!("AppImage updates require Linux");
}

#[cfg(target_os = "linux")]
fn apply_deb(package: &Path) -> Result<()> {
    let status = std::process::Command::new("pkexec")
        .args(["dpkg", "-i", &package.to_string_lossy()])
        .status()
        .context("Could not run pkexec dpkg")?;
    if !status.success() {
        bail!("Package install did not complete (exit {status})");
    }
    let exe = crate::autostart::app_location()?;
    let pid = std::process::id();
    let script = std::env::temp_dir().join("swtrans-relaunch-after-deb.sh");
    let script_body = format!(
        r#"#!/bin/bash
set -e
PID={pid}
EXE={exe:?}
while kill -0 "$PID" 2>/dev/null; do sleep 0.3; done
sleep 1
exec "$EXE" --hidden
"#,
        exe = exe.display(),
    );
    std::fs::write(&script, script_body)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    std::process::Command::new(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Could not schedule relaunch after .deb install")?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn apply_deb(_package: &Path) -> Result<()> {
    bail!(".deb install is only supported on Linux");
}

#[cfg(target_os = "macos")]
fn unzip_app_bundle(archive: &Path, dest: &Path) -> Result<PathBuf> {
    let file = std::fs::File::open(archive).context("Open update zip")?;
    let mut zip = zip::ZipArchive::new(file).context("Read update zip")?;
    zip.extract(dest).context("Extract update zip")?;
    for entry in std::fs::read_dir(dest).context("Read extract dir")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("app") {
            return Ok(path);
        }
    }
    bail!("No .app bundle in update archive");
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

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[test]
    fn picks_windows_x64_asset() {
        let assets = vec![
            GithubAsset {
                name: "swtrans_0.9.0_arm64-setup.exe".into(),
                browser_download_url: "https://example/arm".into(),
            },
            GithubAsset {
                name: "swtrans_0.9.0_x64-setup.exe".into(),
                browser_download_url: "https://example/x64".into(),
            },
        ];
        let picked = pick_asset(&assets).expect("x64 asset");
        assert_eq!(picked.file_name, "swtrans_0.9.0_x64-setup.exe");
        assert_eq!(picked.mode, InstallMode::Nsis);
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn picks_linux_appimage() {
        let assets = vec![GithubAsset {
            name: "swtrans_0.9.0_x86_64.AppImage".into(),
            browser_download_url: "https://example/appimage".into(),
        }];
        // SAFETY: test runs single-threaded; cleared before exit.
        unsafe { std::env::set_var("APPIMAGE", "/tmp/swtrans.AppImage") };
        let picked = pick_asset(&assets).expect("appimage");
        assert_eq!(picked.mode, InstallMode::AppImageReplace);
        unsafe { std::env::remove_var("APPIMAGE") };
    }
}