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
/// 1. Bundled sidecar next to the running executable - our own patched
///    fork ("my-zero": advisor mode's Task tool, plan mode via ACP - see
///    `bundle.externalBin` in tauri.conf.json), built as a
///    `zero-<target-triple>` external binary and renamed to plain
///    `zero`/`zero.exe` by `tauri build`. Checked first so the fork's
///    patches are always what's used, regardless of what the user has on
///    PATH.
/// 2. `zero` on the user's PATH.
/// 3. Isolated zero-desktop cache.
pub fn locate_zero() -> Result<ZeroLocation, LocateError> {
    locate_zero_with_sidecar(sidecar_path())
}

/// `locate_zero()`'s resolution order, with the sidecar path passed in
/// rather than recomputed - lets tests point at an isolated throwaway path
/// instead of the real one next to the test binary, which every test in
/// this process shares (touching it from more than one test would race,
/// since tests run in parallel by default).
fn locate_zero_with_sidecar(sidecar: Option<PathBuf>) -> Result<ZeroLocation, LocateError> {
    // 1. Try the bundled sidecar
    if let Some(path) = sidecar {
        if path.is_file() {
            return Ok(ZeroLocation {
                version: get_version(&path),
                path,
            });
        }
    }

    // 2. Try PATH
    if let Ok(path) = which::which("zero") {
        return Ok(ZeroLocation {
            version: get_version(&path),
            path,
        });
    }

    // 3. Try isolated cache
    let cache = isolated_cache_dir().join("zero");
    if cache.is_file() {
        return Ok(ZeroLocation {
            version: get_version(&cache),
            path: cache,
        });
    }

    Err(LocateError::NotFound)
}

/// Resolves the bundled sidecar path: a file named `zero` (`zero.exe` on
/// Windows) next to the running executable. Mirrors
/// `tauri-plugin-shell`'s `relative_command_path` resolution exactly (that
/// crate isn't a dependency here - `bridge.rs` already spawns `zero` via a
/// plain `tokio::process::Command`, not the shell plugin's sidecar API, so
/// there was no reason to add it) so a `bundle.externalBin` sidecar built
/// by `tauri build` resolves the same way it would through that plugin.
/// Returns `None` only if the running executable's own path can't be
/// determined, which should not happen in practice.
fn sidecar_path() -> Option<PathBuf> {
    let exe_path = std::env::current_exe().ok()?;
    let exe_dir = exe_path.parent()?;
    // `cargo test`/`cargo run` place the binary in `.../target/debug/deps` -
    // go up one level so dev/test builds resolve the same directory a
    // `tauri dev` run would, matching tauri-plugin-shell's own dev-mode
    // special case.
    let base_dir = if exe_dir.ends_with("deps") {
        exe_dir.parent().unwrap_or(exe_dir)
    } else {
        exe_dir
    };
    let mut sidecar = base_dir.join("zero");
    if cfg!(windows) {
        sidecar.set_extension("exe");
    }
    Some(sidecar)
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
            assert!(!version.is_empty(), "version string must not be empty");
        }
    }

    #[test]
    fn test_locate_zero_path_is_absolute() {
        let result = locate_zero().expect("zero CLI must be installed");
        assert!(result.path.is_absolute(), "zero path must be absolute");
    }

    #[test]
    fn test_sidecar_path_sits_next_to_current_exe() {
        let sidecar = sidecar_path().expect("sidecar_path should resolve in a test binary");
        let exe_dir = std::env::current_exe().unwrap();
        let exe_dir = exe_dir.parent().unwrap();
        // Test binaries live in `target/debug/deps` - sidecar_path must go up
        // one level from there, same as tauri-plugin-shell's dev-mode
        // resolution, so it doesn't expect the sidecar inside `deps/`.
        let expected_base = if exe_dir.ends_with("deps") {
            exe_dir.parent().unwrap()
        } else {
            exe_dir
        };
        assert_eq!(sidecar.parent().unwrap(), expected_base);
        let expected_name = if cfg!(windows) { "zero.exe" } else { "zero" };
        assert_eq!(sidecar.file_name().unwrap(), expected_name);
    }

    #[test]
    fn test_locate_zero_with_sidecar_none_falls_through_to_path() {
        // Regression guard for the `.is_file()` check in
        // locate_zero_with_sidecar not short-circuiting on a sidecar path
        // that merely resolves but doesn't exist - a `None` sidecar (or one
        // pointing at a nonexistent file) must fall through to PATH/cache,
        // never be returned as-is.
        let result =
            locate_zero_with_sidecar(None).expect("should fall through to PATH without a sidecar");
        assert_ne!(result.path, PathBuf::from("/nonexistent"));

        let nonexistent = std::env::temp_dir().join("locate-zero-test-nonexistent-sidecar");
        let result = locate_zero_with_sidecar(Some(nonexistent.clone()))
            .expect("should fall through to PATH when the sidecar path doesn't exist");
        assert_ne!(result.path, nonexistent);
    }

    #[test]
    fn test_locate_zero_with_sidecar_prefers_it_over_path() {
        // Isolated per-test temp path - unlike the real sidecar_path()
        // (shared by every test in this process, since it's derived from
        // the one test binary's own location), this can't race with any
        // other test.
        let dir = std::env::temp_dir().join(format!(
            "locate-zero-test-sidecar-{}-{}",
            std::process::id(),
            "prefers-over-path"
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let sidecar = dir.join("zero");
        std::fs::write(&sidecar, b"#!/bin/sh\necho fake-sidecar\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&sidecar).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&sidecar, perms).unwrap();
        }

        let result = locate_zero_with_sidecar(Some(sidecar.clone()));
        std::fs::remove_dir_all(&dir).ok();

        let result =
            result.expect("locate_zero_with_sidecar should succeed with a sidecar present");
        assert_eq!(
            result.path, sidecar,
            "must prefer the bundled sidecar over PATH/cache when it exists"
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
