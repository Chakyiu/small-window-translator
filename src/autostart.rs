use anyhow::{Context, Result, bail};
use auto_launch::AutoLaunchBuilder;
use std::path::{Path, PathBuf};

pub fn is_enabled() -> bool {
    match launcher() {
        Ok(auto) => auto.is_enabled().unwrap_or(false),
        Err(_) => false,
    }
}

pub fn set_enabled(on: bool) -> Result<()> {
    let auto = launcher()?;
    if on {
        auto.enable().context("Could not enable start at login")?;
    } else {
        auto.disable().context("Could not disable start at login")?;
    }
    Ok(())
}

fn launcher() -> Result<auto_launch::AutoLaunch> {
    let path = launch_path()?;
    if !path.exists() {
        bail!("App path does not exist: {}", path.display());
    }
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "swtrans".into());
    AutoLaunchBuilder::new()
        .set_app_name(&name)
        .set_app_path(&path.to_string_lossy())
        .set_args(&["--hidden"])
        .set_use_launch_agent(!is_app_bundle(&path))
        .build()
        .context("Could not configure start at login")
}

fn launch_path() -> Result<PathBuf> {
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        let path = PathBuf::from(appimage);
        if path.exists() {
            return Ok(path);
        }
    }
    let exe = std::env::current_exe().context("Could not find this app")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    Ok(bundle_path(&exe).unwrap_or(exe))
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("app")
}

fn bundle_path(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()?.to_str()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()?.to_str()? != "Contents" {
        return None;
    }
    let app = contents.parent()?;
    is_app_bundle(app).then(|| app.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_app_bundle() {
        let exe = PathBuf::from(
            "/Applications/Small Window Translator.app/Contents/MacOS/swtrans",
        );
        assert_eq!(
            bundle_path(&exe).as_deref(),
            Some(Path::new("/Applications/Small Window Translator.app"))
        );
        assert!(bundle_path(Path::new("/usr/local/bin/swtrans")).is_none());
    }
}
