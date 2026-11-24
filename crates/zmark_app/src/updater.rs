//! Signed release feed handling for the built-in updater.
//!
//! The manifest is verified with Ed25519 before its URL, hash or version are
//! trusted. `notes` is deliberately not part of the signed payload and is not
//! shown in the UI in this release.

use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{run_check, spawn_install_helper};

/// Stable release feed. Kept as a constant so a future mirror can be added
/// without changing the updater's state machine.
pub const UPDATE_FEED_URL: &str =
    "https://github.com/2716190547/zmark/releases/latest/download/latest.json";
const GITEE_RELEASE_API: &str =
    "https://gitee.com/api/v5/repos/zeng_sc/zmark-releases/releases/latest";
const GITEE_RELEASE_BASE: &str = "https://gitee.com/zeng_sc/zmark-releases/releases/download";

fn gitee_latest_feed(metadata: &str, windows: bool) -> Result<(String, String), UpdateError> {
    let value: serde_json::Value =
        serde_json::from_str(metadata).map_err(|error| UpdateError::Parse(error.to_string()))?;
    let tag = str_field(&value, "tag_name")?;
    let version = tag
        .strip_prefix('v')
        .ok_or_else(|| UpdateError::BadVersion(tag.into()))?;
    parse_version(version).ok_or_else(|| UpdateError::BadVersion(version.into()))?;
    let name = if windows {
        "latest-windows.json"
    } else {
        "latest.json"
    };
    Ok((
        format!("{GITEE_RELEASE_BASE}/{tag}/{name}"),
        version.to_string(),
    ))
}

fn gitee_asset_url(manifest: &ReleaseManifest, windows: bool) -> Option<String> {
    parse_version(&manifest.version)?;
    let suffix = if windows { "-setup.exe" } else { ".zip" };
    let name = format!("ZMark-{}{suffix}", manifest.version);
    let expected = format!(
        "https://github.com/2716190547/zmark/releases/download/v{}/{name}",
        manifest.version
    );
    (manifest.url == expected).then(|| format!("{GITEE_RELEASE_BASE}/v{}/{name}", manifest.version))
}

/// Ed25519 public key of the release signing key. Filled by the release setup;
/// an empty key makes every verification fail closed.
pub const UPDATE_PUBLIC_KEY_HEX: &str =
    "d2b3380214d08522d78227e369da57c19f6746ec96135cae65ae85e42053dda5";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub published_at: String,
    pub notes: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    Parse(String),
    MissingField(&'static str),
    BadHex,
    BadSignature,
    BadVersion(String),
    HashMismatch,
    Http(String),
    Io(String),
    NoPublicKey,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "invalid update manifest: {e}"),
            Self::MissingField(field) => write!(f, "update manifest is missing `{field}`"),
            Self::BadHex => write!(f, "update manifest contains invalid hex"),
            Self::BadSignature => write!(f, "update signature verification failed"),
            Self::BadVersion(v) => write!(f, "invalid version `{v}`"),
            Self::HashMismatch => write!(f, "update archive hash does not match"),
            Self::Http(e) => write!(f, "update request failed: {e}"),
            Self::Io(e) => write!(f, "update I/O failed: {e}"),
            Self::NoPublicKey => write!(f, "no update public key configured"),
        }
    }
}

impl std::error::Error for UpdateError {}

fn str_field<'a>(value: &'a serde_json::Value, name: &'static str) -> Result<&'a str, UpdateError> {
    value
        .get(name)
        .and_then(|field| field.as_str())
        .ok_or(UpdateError::MissingField(name))
}

pub fn parse_manifest(json: &str) -> Result<ReleaseManifest, UpdateError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| UpdateError::Parse(error.to_string()))?;
    Ok(ReleaseManifest {
        version: str_field(&value, "version")?.to_string(),
        url: str_field(&value, "url")?.to_string(),
        sha256: str_field(&value, "sha256")?.to_string(),
        published_at: str_field(&value, "published_at")?.to_string(),
        notes: value
            .get("notes")
            .and_then(|notes| notes.as_str())
            .unwrap_or_default()
            .to_string(),
        signature: str_field(&value, "signature")?.to_string(),
    })
}

/// Bytes covered by the Ed25519 signature. Deliberately line-based and without
/// a trailing newline so the release tool and the app can reproduce it exactly.
pub fn signature_payload(manifest: &ReleaseManifest) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        manifest.version, manifest.url, manifest.sha256, manifest.published_at
    )
}

pub fn verify_signature(
    payload: &str,
    signature_hex: &str,
    public_key_hex: &str,
) -> Result<(), UpdateError> {
    if public_key_hex.is_empty() {
        return Err(UpdateError::NoPublicKey);
    }
    let signature = hex::decode(signature_hex).map_err(|_| UpdateError::BadHex)?;
    let public_key = hex::decode(public_key_hex).map_err(|_| UpdateError::BadHex)?;
    if signature.len() != 64 || public_key.len() != 32 {
        return Err(UpdateError::BadSignature);
    }
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key)
        .verify(payload.as_bytes(), &signature)
        .map_err(|_| UpdateError::BadSignature)
}

pub fn verify_manifest_with_key(
    manifest: &ReleaseManifest,
    public_key_hex: &str,
) -> Result<(), UpdateError> {
    verify_signature(
        &signature_payload(manifest),
        &manifest.signature,
        public_key_hex,
    )
}

pub fn verify_manifest(manifest: &ReleaseManifest) -> Result<(), UpdateError> {
    verify_manifest_with_key(manifest, UPDATE_PUBLIC_KEY_HEX)
}

/// Parses a strict three-component numeric version (for example `0.2.0`).
/// Prerelease/build suffixes are rejected because the channel is stable-only.
pub fn parse_version(version: &str) -> Option<[u64; 3]> {
    if version.is_empty() || version.contains('-') || version.contains('+') {
        return None;
    }
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let mut parsed = [0u64; 3];
    for (slot, part) in parsed.iter_mut().zip(parts) {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    Some(parsed)
}

pub fn compare_versions(current: &str, candidate: &str) -> Option<Ordering> {
    Some(parse_version(candidate)?.cmp(&parse_version(current)?))
}

/// Only a strictly newer, well-formed stable version is installable.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    compare_versions(current, candidate) == Some(Ordering::Greater)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(ring::digest::digest(&ring::digest::SHA256, bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    Latest,
    Error(String),
    Downloaded(String),
    Installing,
}

#[derive(Debug, Clone)]
pub struct UpdateConfig {
    pub enabled: bool,
    pub feed_url: String,
    pub public_key_hex: String,
    pub current_version: String,
    pub target_bundle: Option<PathBuf>,
    pub staging_root: Option<PathBuf>,
    pub staged_path: Option<PathBuf>,
    pub disabled_reason: Option<String>,
}

impl UpdateConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            feed_url: UPDATE_FEED_URL.to_string(),
            public_key_hex: UPDATE_PUBLIC_KEY_HEX.to_string(),
            current_version: crate::APP_VERSION.to_string(),
            target_bundle: None,
            staging_root: None,
            staged_path: None,
            disabled_reason: Some("updates are disabled in this build".into()),
        }
    }

    #[cfg(target_os = "windows")]
    pub fn production() -> Self {
        windows::production()
    }

    /// Enables updates only for a writable `.app` bundle outside App
    /// Translocation and removable volumes.
    #[cfg(not(target_os = "windows"))]
    pub fn production() -> Self {
        let mut config = Self::disabled();
        config.public_key_hex = UPDATE_PUBLIC_KEY_HEX.to_string();
        // Local end-to-end testing hook. Signature verification is unchanged,
        // so a forged feed still fails; only the location can be overridden.
        if let Ok(feed) = std::env::var("ZMARK_UPDATE_FEED") {
            config.feed_url = feed;
        }
        match classify_target_bundle(&std::env::current_exe().unwrap_or_default()) {
            None => {
                config.disabled_reason = Some("not running from an installable .app bundle".into());
            }
            Some(bundle) if !parent_is_writable(&bundle) => {
                config.disabled_reason =
                    Some("the app bundle's parent directory is not writable".into());
                config.target_bundle = Some(bundle);
            }
            Some(bundle) => {
                config.enabled = true;
                config.target_bundle = Some(bundle);
                config.disabled_reason = None;
            }
        }
        config
    }

    pub fn testing(
        feed_url: String,
        public_key_hex: String,
        target_bundle: PathBuf,
        staging_root: PathBuf,
        staged_path: PathBuf,
    ) -> Self {
        Self {
            enabled: true,
            feed_url,
            public_key_hex,
            current_version: crate::APP_VERSION.to_string(),
            target_bundle: Some(target_bundle),
            staging_root: Some(staging_root),
            staged_path: Some(staged_path),
            disabled_reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedUpdate {
    pub version: String,
    pub staging_bundle: PathBuf,
    pub sha256: String,
}

/// Resolves `.../X.app/Contents/MacOS/X` to `.../X.app` and rejects paths that
/// cannot be replaced safely.
pub fn classify_target_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()?.to_str()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()?.to_str()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    if bundle.extension().and_then(|ext| ext.to_str()) != Some("app") {
        return None;
    }
    let path = bundle.to_string_lossy();
    if path.contains("AppTranslocation") || bundle.starts_with("/Volumes") {
        return None;
    }
    Some(bundle.to_path_buf())
}

pub fn parent_is_writable(bundle: &Path) -> bool {
    let Some(parent) = bundle.parent() else {
        return false;
    };
    let probe = parent.join(format!(".zmark_write_probe_{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub fn staged_state_path() -> PathBuf {
    zmark_core::config::Preferences::default_config_dir().join("staged.json")
}

pub fn write_staged_state(
    path: &Path,
    version: &str,
    staging_bundle: &Path,
    sha256: &str,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = serde_json::json!({
        "version": version,
        "staging": staging_bundle.to_string_lossy(),
        "sha256": sha256,
    });
    std::fs::write(path, value.to_string())
}

pub fn read_staged_state(path: &Path) -> Option<PreparedUpdate> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let version = value.get("version")?.as_str()?.to_string();
    let staging = PathBuf::from(value.get("staging")?.as_str()?);
    let sha256 = value.get("sha256")?.as_str()?.to_string();
    if !staging.exists() {
        return None;
    }
    Some(PreparedUpdate {
        version,
        staging_bundle: staging,
        sha256,
    })
}

fn run_command(program: &str, args: &[&str]) -> Result<Vec<u8>, UpdateError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| UpdateError::Io(format!("{program}: {error}")))?;
    if !output.status.success() {
        return Err(UpdateError::Http(format!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn download_to(url: &str, path: &Path, max_time: &str, max_size: &str) -> Result<(), UpdateError> {
    let path = path.to_string_lossy().to_string();
    run_command(
        "/usr/bin/curl",
        &[
            "-fsSL",
            "--connect-timeout",
            "10",
            "--max-time",
            max_time,
            "--retry",
            "2",
            "--max-filesize",
            max_size,
            "-o",
            &path,
            url,
        ],
    )?;
    Ok(())
}

fn github_release_api_url(manifest: &ReleaseManifest) -> Option<String> {
    parse_version(&manifest.version)?;
    let expected = format!(
        "https://github.com/2716190547/zmark/releases/download/v{0}/ZMark-{0}.zip",
        manifest.version
    );
    (manifest.url == expected).then(|| {
        format!(
            "https://api.github.com/repos/2716190547/zmark/releases/tags/v{}",
            manifest.version
        )
    })
}

fn github_asset_api_url(
    manifest: &ReleaseManifest,
    metadata: &serde_json::Value,
) -> Option<String> {
    github_release_api_url(manifest)?;
    let expected_name = format!("ZMark-{}.zip", manifest.version);
    let asset = metadata.get("assets")?.as_array()?.iter().find(|asset| {
        asset.get("name").and_then(|v| v.as_str()) == Some(expected_name.as_str())
            && asset.get("browser_download_url").and_then(|v| v.as_str())
                == Some(manifest.url.as_str())
            && asset.get("state").and_then(|v| v.as_str()) == Some("uploaded")
    })?;
    let id = asset.get("id")?.as_u64()?;
    Some(format!(
        "https://api.github.com/repos/2716190547/zmark/releases/assets/{id}"
    ))
}

fn download_github_asset_fallback(
    manifest: &ReleaseManifest,
    path: &Path,
) -> Result<(), UpdateError> {
    let release_url = github_release_api_url(manifest).ok_or_else(|| {
        UpdateError::Http("no trusted GitHub fallback for this release URL".into())
    })?;
    let metadata = run_command(
        "/usr/bin/curl",
        &[
            "-fsSL",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--max-filesize",
            "1048576",
            &release_url,
        ],
    )?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata).map_err(|error| UpdateError::Parse(error.to_string()))?;
    let asset_url = github_asset_api_url(manifest, &metadata)
        .ok_or_else(|| UpdateError::Http("matching GitHub release asset not found".into()))?;
    run_command(
        "/usr/bin/curl",
        &[
            "-fsSL",
            "--connect-timeout",
            "10",
            "--max-time",
            "300",
            "--retry",
            "2",
            "--max-filesize",
            "104857600",
            "-H",
            "Accept: application/octet-stream",
            "-o",
            &path.to_string_lossy(),
            &asset_url,
        ],
    )?;
    Ok(())
}

/// Downloads, verifies and stages an update. Never installs anything: the
/// restart helper performs the swap once the process exits.
#[cfg(not(target_os = "windows"))]
pub fn run_check(config: &UpdateConfig) -> Result<Option<PreparedUpdate>, UpdateError> {
    if !config.enabled {
        return Ok(None);
    }
    let target = config
        .target_bundle
        .as_ref()
        .ok_or_else(|| UpdateError::Io("no target bundle configured".into()))?;
    let feed_path = config
        .staging_root
        .clone()
        .unwrap_or_else(|| target.parent().unwrap_or(Path::new(".")).to_path_buf())
        .join(format!(".ZMark.feed.{}.json", std::process::id()));
    if let Some(parent) = feed_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| UpdateError::Io(e.to_string()))?;
    }
    let mut mirror_version = None;
    if let Err(primary_error) = download_to(&config.feed_url, &feed_path, "30", "65536") {
        if config.feed_url != UPDATE_FEED_URL {
            return Err(primary_error);
        }
        download_to(GITEE_RELEASE_API, &feed_path, "30", "1048576")?;
        let metadata =
            std::fs::read_to_string(&feed_path).map_err(|e| UpdateError::Io(e.to_string()))?;
        let (url, version) = gitee_latest_feed(&metadata, false)?;
        download_to(&url, &feed_path, "30", "65536")?;
        mirror_version = Some(version);
    }
    let feed = std::fs::read_to_string(&feed_path).map_err(|e| UpdateError::Io(e.to_string()))?;
    let _ = std::fs::remove_file(&feed_path);
    let manifest = parse_manifest(&feed)?;
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

    let staging_root = config.staging_root.clone().unwrap_or_else(|| {
        target
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!(".ZMark.staging.{}", std::process::id()))
    });
    let _ = std::fs::remove_dir_all(&staging_root);
    std::fs::create_dir_all(&staging_root).map_err(|e| UpdateError::Io(e.to_string()))?;

    let zip_path = staging_root.join("update.zip");
    let mirror_url = gitee_asset_url(&manifest, false);
    let primary_url = if mirror_version.is_some() {
        mirror_url.as_deref().ok_or_else(|| {
            UpdateError::Http("no trusted Gitee mirror for this release URL".into())
        })?
    } else {
        &manifest.url
    };
    if let Err(direct_error) = download_to(primary_url, &zip_path, "300", "104857600") {
        let alternate = if mirror_version.is_some() {
            Some(manifest.url.as_str())
        } else {
            mirror_url.as_deref()
        };
        let alternate_result = alternate
            .ok_or_else(|| UpdateError::Http("no trusted alternate release URL".into()))
            .and_then(|url| download_to(url, &zip_path, "300", "104857600"));
        if let Err(mirror_error) = alternate_result {
            download_github_asset_fallback(&manifest, &zip_path).map_err(|fallback_error| {
                UpdateError::Http(format!("direct download failed ({direct_error}); Gitee failed ({mirror_error}); GitHub API failed ({fallback_error})"))
            })?;
        }
    }
    let zip_bytes = std::fs::read(&zip_path).map_err(|e| UpdateError::Io(e.to_string()))?;
    if sha256_hex(&zip_bytes) != manifest.sha256.to_lowercase() {
        let _ = std::fs::remove_dir_all(&staging_root);
        return Err(UpdateError::HashMismatch);
    }

    let extracted = staging_root.join("extracted");
    std::fs::create_dir_all(&extracted).map_err(|e| UpdateError::Io(e.to_string()))?;
    run_command(
        "/usr/bin/ditto",
        &[
            "-x",
            "-k",
            &zip_path.to_string_lossy(),
            &extracted.to_string_lossy(),
        ],
    )?;
    let staging_bundle = extracted.join("ZMark.app");
    if !staging_bundle.is_dir() {
        let _ = std::fs::remove_dir_all(&staging_root);
        return Err(UpdateError::Io("archive does not contain ZMark.app".into()));
    }
    let executable = staging_bundle.join("Contents/MacOS/ZMark");
    if !executable.is_file() {
        let _ = std::fs::remove_dir_all(&staging_root);
        return Err(UpdateError::Io("staged bundle has no executable".into()));
    }
    let plist = staging_bundle.join("Contents/Info.plist");
    let identifier = run_command(
        "/usr/libexec/PlistBuddy",
        &["-c", "Print :CFBundleIdentifier", &plist.to_string_lossy()],
    )?;
    if String::from_utf8_lossy(&identifier).trim() != "com.zmark.app" {
        let _ = std::fs::remove_dir_all(&staging_root);
        return Err(UpdateError::Io("unexpected bundle identifier".into()));
    }
    let staged_version = run_command(
        "/usr/libexec/PlistBuddy",
        &[
            "-c",
            "Print :CFBundleShortVersionString",
            &plist.to_string_lossy(),
        ],
    )?;
    if String::from_utf8_lossy(&staged_version).trim() != manifest.version {
        let _ = std::fs::remove_dir_all(&staging_root);
        return Err(UpdateError::BadVersion(manifest.version));
    }
    run_command(
        "/usr/bin/codesign",
        &[
            "--verify",
            "--deep",
            "--strict",
            &staging_bundle.to_string_lossy(),
        ],
    )?;
    run_command(
        "/usr/bin/xattr",
        &[
            "-dr",
            "com.apple.quarantine",
            &staging_bundle.to_string_lossy(),
        ],
    )?;
    let _ = std::fs::remove_file(&zip_path);

    let prepared = PreparedUpdate {
        version: manifest.version,
        staging_bundle,
        sha256: manifest.sha256,
    };
    let staged_path = config.staged_path.clone().unwrap_or_else(staged_state_path);
    let _ = write_staged_state(
        &staged_path,
        &prepared.version,
        &prepared.staging_bundle,
        &prepared.sha256,
    );
    Ok(Some(prepared))
}

/// Starts the detached install helper. It waits for this process to exit, so
/// the caller must quit only after the dirty-document flow is resolved.
#[cfg(not(target_os = "windows"))]
pub fn spawn_install_helper(prepared: &PreparedUpdate, target: &Path) -> std::io::Result<()> {
    let script_dir = prepared
        .staging_bundle
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no staging dir"))?;
    let script_path = script_dir.join("install.sh");
    std::fs::write(&script_path, install_script())?;
    let mut command = Command::new("/bin/sh");
    command
        .arg(&script_path)
        .arg(target)
        .arg(std::process::id().to_string())
        .arg(&prepared.version)
        .arg(&prepared.staging_bundle);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn()?;
    Ok(())
}

/// Shell helper that waits for the app to exit and then swaps the staged bundle
/// into place. `ZMARK_OPEN_CMD` exists so tests can record the `open` call.
pub fn install_script() -> &'static str {
    r#"#!/bin/sh
set -u
target="$1"; pid="$2"; version="$3"; staging="$4"
backup="$target.old.$version"
open_cmd="${ZMARK_OPEN_CMD:-open}"
while kill -0 "$pid" 2>/dev/null; do sleep 0.1; done
restore() {
  if [ ! -d "$target" ]; then
    if [ -d "$backup" ]; then
      mv "$backup" "$target"
    else
      latest=$(ls -dt "$target".old.* 2>/dev/null | head -n1)
      [ -n "$latest" ] && mv "$latest" "$target"
    fi
  fi
}
trap restore EXIT
trap 'trap - EXIT; restore; "$open_cmd" "$target" 2>/dev/null || true; exit 1' INT TERM
if [ ! -d "$target" ]; then
  latest=$(ls -dt "$target".old.* 2>/dev/null | head -n1)
  [ -n "$latest" ] && mv "$latest" "$target"
fi
[ -d "$target" ] || exit 1
rm -rf "$backup"
if ! mv "$target" "$backup"; then
  "$open_cmd" "$target" 2>/dev/null || true
  exit 1
fi
if [ -n "${ZMARK_TEST_PAUSE_AFTER_BACKUP:-}" ]; then sleep "$ZMARK_TEST_PAUSE_AFTER_BACKUP"; fi
for b in "$target".old.*; do
  [ "$b" = "$backup" ] || rm -rf "$b"
done
if ! mv "$staging" "$target"; then
  mv "$backup" "$target"
  "$open_cmd" "$target" 2>/dev/null || true
  exit 1
fi
trap - EXIT INT TERM
xattr -dr com.apple.quarantine "$target" 2>/dev/null || true
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$target" 2>/dev/null || true
"$open_cmd" "$target"
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitee_urls_accept_only_stable_tags_and_signed_github_asset_paths() {
        let (url, version) = gitee_latest_feed(r#"{"tag_name":"v0.2.5"}"#, true).unwrap();
        assert_eq!(version, "0.2.5");
        assert_eq!(
            url,
            "https://gitee.com/zeng_sc/zmark-releases/releases/download/v0.2.5/latest-windows.json"
        );
        assert!(gitee_latest_feed(r#"{"tag_name":"v../../evil"}"#, false).is_err());
        let (mut manifest, _) = signed_manifest();
        manifest.version = version;
        manifest.url = "https://github.com/2716190547/zmark/releases/download/v0.2.5/ZMark-0.2.5-setup.exe".into();
        assert_eq!(gitee_asset_url(&manifest, true).as_deref(), Some("https://gitee.com/zeng_sc/zmark-releases/releases/download/v0.2.5/ZMark-0.2.5-setup.exe"));
        assert!(gitee_asset_url(&manifest, false).is_none());
        manifest.url.push_str("?redirect=evil");
        assert!(gitee_asset_url(&manifest, true).is_none());
    }

    #[test]
    fn github_fallback_only_accepts_the_signed_release_asset() {
        let (mut manifest, _) = signed_manifest();
        manifest.version = "0.2.2".into();
        manifest.url =
            "https://github.com/2716190547/zmark/releases/download/v0.2.2/ZMark-0.2.2.zip"
                .into();
        let metadata = serde_json::json!({"assets": [{"id": 726074161u64, "name": "ZMark-0.2.2.zip", "browser_download_url": manifest.url, "state": "uploaded"}]});
        assert_eq!(
            github_asset_api_url(&manifest, &metadata).as_deref(),
            Some(
                "https://api.github.com/repos/2716190547/zmark/releases/assets/726074161"
            )
        );
        manifest.url.push_str("?unexpected=1");
        assert!(github_asset_api_url(&manifest, &metadata).is_none());
        manifest.url = "https://example.com/ZMark-0.2.2.zip".into();
        assert!(github_asset_api_url(&manifest, &metadata).is_none());
    }
    use ring::signature::KeyPair;

    fn signed_manifest() -> (ReleaseManifest, String) {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let keypair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let public_key = hex::encode(keypair.public_key().as_ref());
        let mut manifest = ReleaseManifest {
            version: "0.2.1".into(),
            url: "https://example.com/ZMark-0.2.1.zip".into(),
            sha256: "00".repeat(32),
            published_at: "2026-11-06T00:00:00Z".into(),
            notes: String::new(),
            signature: String::new(),
        };
        let payload = signature_payload(&manifest);
        manifest.signature = hex::encode(keypair.sign(payload.as_bytes()).as_ref());
        (manifest, public_key)
    }

    #[test]
    fn version_ordering_is_numeric_and_strict() {
        assert!(is_newer("0.9.0", "0.10.0"));
        assert!(is_newer("0.2.0", "0.2.1"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.2.1", "0.2.0"));
        assert!(!is_newer("0.2.0", "v0.2.1"));
        assert!(!is_newer("0.2.0", "0.2.1-beta"));
        assert!(!is_newer("0.2.0", "0.2"));
        assert!(!is_newer("0.2.0", "0.2.x"));
    }

    #[test]
    fn manifest_round_trips_and_notes_are_optional() {
        let json = r#"{
            "version": "0.2.1",
            "url": "https://example.com/ZMark.zip",
            "sha256": "abc",
            "published_at": "2026-11-06T00:00:00Z",
            "signature": "deadbeef"
        }"#;
        let manifest = parse_manifest(json).unwrap();
        assert_eq!(manifest.version, "0.2.1");
        assert_eq!(manifest.notes, "");
        assert_eq!(manifest.signature, "deadbeef");

        let missing = r#"{"version":"0.2.1","url":"u","sha256":"h","published_at":"p"}"#;
        assert_eq!(
            parse_manifest(missing).unwrap_err(),
            UpdateError::MissingField("signature")
        );
    }

    #[test]
    fn signature_payload_is_exact() {
        let manifest = ReleaseManifest {
            version: "0.2.1".into(),
            url: "u".into(),
            sha256: "h".into(),
            published_at: "p".into(),
            notes: String::new(),
            signature: String::new(),
        };
        assert_eq!(signature_payload(&manifest), "0.2.1\nu\nh\np");
    }

    #[test]
    fn ed25519_verification_accepts_original_and_rejects_tampering() {
        let (manifest, public_key) = signed_manifest();
        verify_manifest_with_key(&manifest, &public_key).unwrap();
        assert_eq!(
            verify_manifest_with_key(&manifest, "").unwrap_err(),
            UpdateError::NoPublicKey
        );

        let mut tampered = manifest.clone();
        tampered.sha256 = "11".repeat(32);
        assert_eq!(
            verify_manifest_with_key(&tampered, &public_key).unwrap_err(),
            UpdateError::BadSignature
        );

        let mut truncated = manifest.clone();
        truncated.signature.truncate(20);
        assert_eq!(
            verify_manifest_with_key(&truncated, &public_key).unwrap_err(),
            UpdateError::BadSignature
        );

        let mut bad_hex = manifest.clone();
        bad_hex.signature = "zz".into();
        assert_eq!(
            verify_manifest_with_key(&bad_hex, &public_key).unwrap_err(),
            UpdateError::BadHex
        );
    }

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "zmark_updater_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_fake_app(root: &Path, version: &str) -> PathBuf {
        let app = root.join("ZMark.app");
        let macos = app.join("Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        let executable = macos.join("ZMark");
        std::fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&executable, permissions).unwrap();
        }
        std::fs::write(
            app.join("Contents/Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.zmark.app</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
<key>CFBundleExecutable</key><string>ZMark</string>
</dict></plist>"#
            ),
        )
        .unwrap();
        app
    }

    #[cfg(target_os = "macos")]
    fn ad_hoc_sign(app: &Path) {
        let status = Command::new("/usr/bin/codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(app)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn sign_with_test_key(manifest: &mut ReleaseManifest) -> String {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let keypair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let public_key = hex::encode(keypair.public_key().as_ref());
        let payload = signature_payload(manifest);
        manifest.signature = hex::encode(keypair.sign(payload.as_bytes()).as_ref());
        public_key
    }

    fn write_manifest(path: &Path, manifest: &ReleaseManifest) {
        let value = serde_json::json!({
            "version": manifest.version,
            "url": manifest.url,
            "sha256": manifest.sha256,
            "published_at": manifest.published_at,
            "notes": manifest.notes,
            "signature": manifest.signature,
        });
        std::fs::write(path, value.to_string()).unwrap();
    }

    #[test]
    fn target_bundle_classification_rejects_unsafe_locations() {
        let exe = Path::new("/Applications/ZMark.app/Contents/MacOS/ZMark");
        assert_eq!(
            classify_target_bundle(exe),
            Some(PathBuf::from("/Applications/ZMark.app"))
        );
        assert_eq!(
            classify_target_bundle(Path::new(
                "/private/var/folders/AppTranslocation/ZMark.app/Contents/MacOS/ZMark"
            )),
            None
        );
        assert_eq!(
            classify_target_bundle(Path::new("/Volumes/USB/ZMark.app/Contents/MacOS/ZMark")),
            None
        );
        assert_eq!(
            classify_target_bundle(Path::new("/tmp/zmark_app/target/debug/zmark_app")),
            None
        );
    }

    #[test]
    fn staged_state_round_trips() {
        let root = temp_root("staged");
        let app = make_fake_app(&root.join("staging"), "9.9.9");
        let staged = root.join("staged.json");
        write_staged_state(&staged, "9.9.9", &app, "abc").unwrap();
        let read = read_staged_state(&staged).unwrap();
        assert_eq!(read.version, "9.9.9");
        assert_eq!(read.staging_bundle, app);
        assert_eq!(read.sha256, "abc");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_script_contains_recovery_guards() {
        let script = install_script();
        assert!(script.contains("trap restore EXIT"));
        assert!(script.contains("trap - EXIT; restore"));
        assert!(script.contains("open_cmd="));
        assert!(script.contains("[ -d \"$target\" ] || exit 1"));
        assert!(script.contains("ZMARK_TEST_PAUSE_AFTER_BACKUP"));
        assert!(script.contains("lsregister"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn file_feed_stages_a_verified_bundle_and_rejects_bad_hashes() {
        let root = temp_root("e2e");
        let source_app = make_fake_app(&root.join("source"), "9.9.9");
        ad_hoc_sign(&source_app);
        let zip = root.join("ZMark-9.9.9.zip");
        let status = Command::new("/usr/bin/ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(&source_app)
            .arg(&zip)
            .status()
            .unwrap();
        assert!(status.success());
        let mut manifest = ReleaseManifest {
            version: "9.9.9".into(),
            url: format!("file://{}", zip.display()),
            sha256: sha256_hex(&std::fs::read(&zip).unwrap()),
            published_at: "2026-11-06T00:00:00Z".into(),
            notes: String::new(),
            signature: String::new(),
        };
        let public_key = sign_with_test_key(&mut manifest);
        let feed = root.join("latest.json");
        write_manifest(&feed, &manifest);

        let target = root.join("Applications/ZMark.app");
        let config = UpdateConfig::testing(
            format!("file://{}", feed.display()),
            public_key,
            target,
            root.join("staging"),
            root.join("staged.json"),
        );
        let prepared = run_check(&config).unwrap().expect("a newer version");
        assert_eq!(prepared.version, "9.9.9");
        assert!(prepared
            .staging_bundle
            .join("Contents/MacOS/ZMark")
            .is_file());
        assert!(root.join("staged.json").is_file());

        let mut tampered = manifest.clone();
        tampered.sha256 = "00".repeat(32);
        let tampered_key = sign_with_test_key(&mut tampered);
        let tampered_feed = root.join("tampered.json");
        write_manifest(&tampered_feed, &tampered);
        assert_eq!(
            run_check(&UpdateConfig::testing(
                format!("file://{}", tampered_feed.display()),
                tampered_key,
                config.target_bundle.clone().unwrap(),
                root.join("staging2"),
                root.join("staged2.json"),
            ))
            .unwrap_err(),
            UpdateError::HashMismatch
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn install_helper_swaps_normally_and_restores_on_sigterm() {
        let root = temp_root("helper");
        let target = make_fake_app(&root.join("target"), "0.2.0");
        let staged = make_fake_app(&root.join("staging"), "9.9.9");
        let script = root.join("install.sh");
        std::fs::write(&script, install_script()).unwrap();
        let marker = root.join("opened.txt");
        let open_script = root.join("open.sh");
        std::fs::write(
            &open_script,
            format!("#!/bin/sh\necho \"$1\" >> \"{}\"\n", marker.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&open_script).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&open_script, permissions).unwrap();
        }

        // Normal swap: the helper waits for the fake app PID, then replaces it.
        // Reap the sleeper from another thread: a zombie would keep `kill -0`
        // succeeding and the helper would wait forever.
        let mut sleeper = Command::new("/bin/sh")
            .args(["-c", "sleep 0.2"])
            .spawn()
            .unwrap();
        let sleeper_pid = sleeper.id();
        let reaper = std::thread::spawn(move || {
            let _ = sleeper.wait();
        });
        let status = Command::new("/bin/sh")
            .arg(&script)
            .arg(&target)
            .arg(sleeper_pid.to_string())
            .arg("9.9.9")
            .arg(&staged)
            .env("ZMARK_OPEN_CMD", &open_script)
            .status()
            .unwrap();
        let _ = reaper.join();
        assert!(status.success());
        assert!(target.join("Contents/MacOS/ZMark").is_file());
        assert!(marker.is_file());
        assert!(root.join("target/ZMark.app.old.9.9.9").is_dir());

        // SIGTERM inside the two-mv window: the trap restores the old bundle.
        let target2 = make_fake_app(&root.join("target2"), "0.2.0");
        let staged2 = make_fake_app(&root.join("staging2"), "9.9.9");
        let marker2 = root.join("opened2.txt");
        let open_script2 = root.join("open2.sh");
        std::fs::write(
            &open_script2,
            format!("#!/bin/sh\necho \"$1\" >> \"{}\"\n", marker2.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&open_script2).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&open_script2, permissions).unwrap();
        }
        let mut sleeper2 = Command::new("/bin/sh")
            .args(["-c", "sleep 0.2"])
            .spawn()
            .unwrap();
        let sleeper2_pid = sleeper2.id();
        let reaper2 = std::thread::spawn(move || {
            let _ = sleeper2.wait();
        });
        let mut helper = Command::new("/bin/sh")
            .arg(&script)
            .arg(&target2)
            .arg(sleeper2_pid.to_string())
            .arg("9.9.9")
            .arg(&staged2)
            .env("ZMARK_OPEN_CMD", &open_script2)
            .env("ZMARK_TEST_PAUSE_AFTER_BACKUP", "5")
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &helper.id().to_string()])
            .status();
        let status = helper.wait().unwrap();
        let _ = reaper2.join();
        assert!(!status.success());
        assert!(
            target2.join("Contents/MacOS/ZMark").is_file(),
            "trap must restore the old bundle"
        );
        assert!(
            !target2.join("ZMark.app").exists(),
            "staging must not be nested inside the restored bundle"
        );
        assert!(marker2.is_file(), "restored app must be opened");
        let _ = std::fs::remove_dir_all(&root);
    }
}
