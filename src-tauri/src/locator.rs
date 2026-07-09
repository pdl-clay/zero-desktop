use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of locating the zero CLI binary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ZeroLocation {
    pub path: PathBuf,
    pub version: Option<String>,
}

/// Error type for zero CLI location failures.
#[derive(Debug, thiserror::Error)]
pub enum LocateError {
    #[error("zero CLI not found on PATH or in cache")]
    NotFound,
    #[error("failed to execute zero --version: {0}")]
    #[allow(dead_code)]
    VersionCheck(String),
}

/// Locate the zero CLI binary.
///
/// Resolution order:
/// 1. `zero` on the user's PATH.
/// 2. Isolated zero-desktop cache.
/// 3. (Future) trigger installation assistant.
pub fn locate_zero() -> Result<ZeroLocation, LocateError> {
    // 1. Try PATH
    if let Ok(path) = which::which("zero") {
        return Ok(ZeroLocation {
            version: get_version(&path),
            path,
        });
    }

    // 2. Try isolated cache
    let cache = isolated_cache_dir().join("zero");
    if cache.is_file() {
        return Ok(ZeroLocation {
            version: get_version(&cache),
            path: cache,
        });
    }

    Err(LocateError::NotFound)
}

/// Get the directory of the isolated zero-desktop cache.
pub fn isolated_cache_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("zero-desktop").join("bin")
}

/// Run `zero --version` and capture the version string.
fn get_version(binary: &Path) -> Option<String> {
    Command::new(binary)
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Add the `which` crate dependency.
/// This function exists only to document the dependency; it is not used directly.
#[allow(dead_code)]
fn _which_dep() {}

/// Add the `dirs` crate dependency.
#[allow(dead_code)]
fn _dirs_dep() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locate_zero_works() {
        let result = locate_zero();
        assert!(
            result.is_ok(),
            "zero CLI must be installed to run this test"
        );

        let location = result.unwrap();
        assert!(location.path.is_file(), "zero path must point to a file");

        if let Some(version) = &location.version {
            assert!(
                !version.is_empty(),
                "version string must not be empty"
            );
        }
    }

    #[test]
    fn test_locate_zero_path_is_absolute() {
        let result = locate_zero().expect("zero CLI must be installed");
        assert!(
            result.path.is_absolute(),
            "zero path must be absolute"
        );
    }

    #[test]
    fn test_isolated_cache_dir_exists() {
        let cache = isolated_cache_dir();
        assert!(
            cache.ends_with("zero-desktop/bin"),
            "cache dir must end with zero-desktop/bin, got: {:?}",
            cache
        );
    }

    #[test]
    fn test_zero_location_serialization_roundtrip() {
        let original = ZeroLocation {
            path: PathBuf::from("/usr/local/bin/zero"),
            version: Some("1.0.0".to_string()),
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: ZeroLocation = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.path, original.path);
        assert_eq!(parsed.version, original.version);
    }

    #[test]
    fn test_zero_location_no_version_serialization() {
        let original = ZeroLocation {
            path: PathBuf::from("/tmp/zero"),
            version: None,
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: ZeroLocation = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.path, original.path);
        assert_eq!(parsed.version, None);
    }

    #[test]
    fn test_locate_error_display() {
        assert_eq!(
            LocateError::NotFound.to_string(),
            "zero CLI not found on PATH or in cache"
        );

        let version_err = LocateError::VersionCheck("timeout".to_string());
        assert_eq!(
            version_err.to_string(),
            "failed to execute zero --version: timeout"
        );
    }
}
