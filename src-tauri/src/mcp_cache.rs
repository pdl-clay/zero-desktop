use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Status cache entry for a single MCP backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedStatus {
    pub status: String,
    #[serde(rename = "toolCount", default)]
    pub tool_count: i64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(rename = "checkedAt", default)]
    pub checked_at: Option<u64>,
}

/// Persistent cache of MCP backend health checks.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McpStatusCache {
    #[serde(default)]
    pub servers: HashMap<String, CachedStatus>,
    #[serde(rename = "generatedAt", default)]
    pub generated_at: Option<u64>,
}

thread_local! {
    static OVERRIDE_PATH: std::cell::RefCell<Option<PathBuf>> = std::cell::RefCell::new(None);
}

/// Path to the cache file inside the app's data directory.
pub fn cache_path() -> Result<PathBuf, String> {
    if let Some(overridden) = OVERRIDE_PATH.with(|p| p.borrow().clone()) {
        return Ok(overridden);
    }
    let base = dirs::data_dir().ok_or_else(|| "Could not resolve app data directory".to_string())?;
    Ok(base.join("zero-desktop").join("mcp-status-cache.json"))
}

/// Load the cache from disk, returning an empty cache if missing or corrupt.
pub fn load() -> McpStatusCache {
    let path = match cache_path() {
        Ok(p) => p,
        Err(_) => return McpStatusCache::default(),
    };

    if !path.exists() {
        return McpStatusCache::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => McpStatusCache::default(),
    }
}

/// Save the cache to disk.
pub fn save(cache: &McpStatusCache) -> Result<(), String> {
    let path = cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(cache).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Update the cached status for a single server and persist.
pub fn set_status(name: &str, status: &str, tool_count: i64, error: Option<String>) {
    let mut cache = load();
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .ok();
    cache.servers.insert(
        name.to_string(),
        CachedStatus {
            status: status.to_string(),
            tool_count,
            error,
            checked_at: now,
        },
    );
    if cache.generated_at.is_none() {
        cache.generated_at = now;
    }
    let _ = save(&cache);
}

/// Remove a server from the cache.
pub fn remove(name: &str) {
    let mut cache = load();
    cache.servers.remove(name);
    let _ = save(&cache);
}

/// Clear the entire cache.
pub fn clear() {
    let _ = save(&McpStatusCache::default());
}

/// Check whether the cache file exists on disk.
pub fn exists() -> bool {
    cache_path().map(|p| p.exists()).unwrap_or(false)
}

/// Return a copy of the currently cached status for a server, if any.
pub fn get(name: &str) -> Option<CachedStatus> {
    load().servers.get(name).cloned()
}

/// Return a copy of all cached statuses.
pub fn all() -> HashMap<String, CachedStatus> {
    load().servers.clone()
}

/// Return the path of the cache file for debugging/frontend display.
pub fn path() -> Result<PathBuf, String> {
    cache_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn set_test_path() -> (PathBuf, PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!(
            "zero-desktop-test-{}-mcp-cache",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache_file = temp_dir.join("zero-desktop").join("mcp-status-cache.json");
        OVERRIDE_PATH.with(|p| *p.borrow_mut() = Some(cache_file.clone()));
        (temp_dir, cache_file)
    }

    fn reset_test_path() {
        OVERRIDE_PATH.with(|p| *p.borrow_mut() = None);
    }

    fn cleanup(temp_dir: PathBuf) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        reset_test_path();
    }

    #[test]
    fn test_save_and_load() {
        let (temp_dir, _cache_file) = set_test_path();
        set_status("brave-search", "ok", 3, None);
        set_status("filesystem", "error", 0, Some("connection refused".to_string()));

        let cache = load();
        assert_eq!(cache.servers.len(), 2);
        assert_eq!(cache.servers["brave-search"].status, "ok");
        assert_eq!(cache.servers["brave-search"].tool_count, 3);
        assert_eq!(cache.servers["filesystem"].status, "error");
        assert!(cache.servers["filesystem"].error.is_some());

        cleanup(temp_dir);
    }

    #[test]
    fn test_remove() {
        let (temp_dir, _cache_file) = set_test_path();
        set_status("server-a", "ok", 1, None);
        remove("server-a");
        let cache = load();
        assert!(!cache.servers.contains_key("server-a"));

        cleanup(temp_dir);
    }

    #[test]
    fn test_get() {
        let (temp_dir, _cache_file) = set_test_path();
        set_status("server-b", "ok", 2, None);
        let cached = get("server-b").unwrap();
        assert_eq!(cached.status, "ok");
        assert_eq!(cached.tool_count, 2);

        cleanup(temp_dir);
    }

    #[test]
    fn test_all() {
        let (temp_dir, _cache_file) = set_test_path();
        set_status("server-c", "ok", 1, None);
        let all = all();
        assert!(all.contains_key("server-c"));

        cleanup(temp_dir);
    }

    #[test]
    fn test_clear() {
        let (temp_dir, _cache_file) = set_test_path();
        set_status("server-d", "ok", 1, None);
        clear();
        let cache = load();
        assert!(cache.servers.is_empty());

        cleanup(temp_dir);
    }

    #[test]
    fn test_missing_get() {
        let (temp_dir, _cache_file) = set_test_path();
        assert!(get("unknown-server").is_none());

        cleanup(temp_dir);
    }
}
