use super::*;
use std::os::windows::process::CommandExt;

const WINDOWS_FEED: &str =
    "https://github.com/2716190547/zmark-releases/releases/latest/download/latest-windows.json";
const NO_WINDOW: u32 = 0x08000000;

pub(super) fn production() -> UpdateConfig {
    let mut config = UpdateConfig::disabled();
    config.feed_url = std::env::var("ZMARK_UPDATE_FEED").unwrap_or_else(|_| WINDOWS_FEED.into());
    let exe = std::env::current_exe().unwrap_or_default();
    let installed = exe
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("ZMark.exe"))
        && exe
            .parent()
            .is_some_and(|dir| dir.join("Uninstall.exe").is_file());
    if installed {
        config.target_bundle = exe.parent().map(Path::to_path_buf);
        config.enabled = true;
        config.disabled_reason = None;
    } else {
        config.disabled_reason =
            Some("automatic updates require the installed Windows version".into());
    }
    config
}

fn curl(url: &str, path: &Path, max_size: &str, max_time: &str) -> Result<(), UpdateError> {
    let system = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or_else(|| UpdateError::Io("SystemRoot is not set".into()))?;
    let output = Command::new(system.join("System32").join("curl.exe"))
        .args([
            "-fsSL",
            "--connect-timeout",
            "10",
            "--max-time",
            max_time,
            "--max-filesize",
            max_size,
            "-o",
        ])
        .arg(path)
        .arg(url)
        .creation_flags(NO_WINDOW)
        .output()
        .map_err(|e| UpdateError::Io(e.to_string()))?;
    if !output.status.success() {
        return Err(UpdateError::Http(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(())
}

pub fn run_check(config: &UpdateConfig) -> Result<Option<PreparedUpdate>, UpdateError> {
    if !config.enabled {
        return Ok(None);
    }
    let staging = tempfile::Builder::new()
        .prefix("zmark-update-")
        .tempdir()
        .map_err(|e| UpdateError::Io(e.to_string()))?;
    let feed = staging.path().join("latest.json");
    let mut mirror_version = None;
    if let Err(primary_error) = curl(&config.feed_url, &feed, "65536", "30") {
        if config.feed_url != WINDOWS_FEED {
            return Err(primary_error);
        }
        curl(GITEE_RELEASE_API, &feed, "1048576", "30")?;
        let metadata =
            std::fs::read_to_string(&feed).map_err(|e| UpdateError::Io(e.to_string()))?;
        let (url, version) = gitee_latest_feed(&metadata, true)?;
        curl(&url, &feed, "65536", "30")?;
        mirror_version = Some(version);
    }
    let manifest = parse_manifest(
        &std::fs::read_to_string(feed).map_err(|e| UpdateError::Io(e.to_string()))?,
    )?;
    verify_manifest_with_key(&manifest, &config.public_key_hex)?;
    if mirror_version
        .as_deref()
        .is_some_and(|version| version != manifest.version)
    {
        return Err(UpdateError::BadVersion(manifest.version));
    }
    if !is_newer(&config.current_version, &manifest.version) {
        return Ok(None);
    }
    let expected_url = format!(
        "https://github.com/2716190547/zmark-releases/releases/download/v{0}/ZMark-{0}-setup.exe",
        manifest.version
    );
    if manifest.url != expected_url {
        return Err(UpdateError::Http("unexpected Windows installer URL".into()));
    }
    let installer = staging.path().join("setup.exe");
    let mirror_url = gitee_asset_url(&manifest, true)
        .ok_or_else(|| UpdateError::Http("no trusted Gitee mirror for this release URL".into()))?;
    let (primary, alternate) = if mirror_version.is_some() {
        (mirror_url.as_str(), manifest.url.as_str())
    } else {
        (manifest.url.as_str(), mirror_url.as_str())
    };
    if let Err(primary_error) = curl(primary, &installer, "104857600", "300") {
        curl(alternate, &installer, "104857600", "300").map_err(|mirror_error| {
            UpdateError::Http(format!(
                "direct download failed ({primary_error}); Gitee failed ({mirror_error})"
            ))
        })?;
    }
    let bytes = std::fs::read(&installer).map_err(|e| UpdateError::Io(e.to_string()))?;
    if sha256_hex(&bytes) != manifest.sha256.to_lowercase() {
        return Err(UpdateError::HashMismatch);
    }
    let path = staging.keep().join("setup.exe");
    Ok(Some(PreparedUpdate {
        version: manifest.version,
        staging_bundle: path,
        sha256: manifest.sha256,
    }))
}

pub fn spawn_install_helper(prepared: &PreparedUpdate, target: &Path) -> std::io::Result<()> {
    let bytes = std::fs::read(&prepared.staging_bundle)?;
    if sha256_hex(&bytes) != prepared.sha256.to_lowercase() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "installer hash mismatch",
        ));
    }
    fn ps_quote(path: &Path) -> String {
        format!("'{}'", path.to_string_lossy().replace('\'', "''"))
    }
    let system = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "SystemRoot is not set")
        })?;
    let powershell = system.join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script = format!(
        "$installer={}; $dir={}; $expected='{}'; $app=Join-Path $dir 'ZMark.exe'; Wait-Process -Id {} -ErrorAction SilentlyContinue; $file=$null; try {{ $file=[System.IO.File]::Open($installer,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::Read); $sha=[System.Security.Cryptography.SHA256]::Create(); try {{ $hash=([BitConverter]::ToString($sha.ComputeHash($file))).Replace('-','').ToLowerInvariant() }} finally {{ $sha.Dispose() }}; if ($hash -eq $expected) {{ Start-Process -FilePath $installer -Verb RunAs -ArgumentList ('/S /D=' + $dir) -Wait | Out-Null }} }} catch {{ }} finally {{ if ($file) {{ $file.Dispose() }} }}; if (Test-Path -LiteralPath $app) {{ Start-Process -FilePath $app }}",
        ps_quote(&prepared.staging_bundle), ps_quote(target), prepared.sha256.to_lowercase(), std::process::id()
    );
    Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(NO_WINDOW)
        .spawn()?;
    Ok(())
}
