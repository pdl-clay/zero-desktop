import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Locate the zero CLI binary on the system.
 * @returns {Promise<{ path: string, version: string | null }>}
 */
export async function locateZeroCli() {
  return invoke("locate_zero_cli");
}

/**
 * Start or resume a zero session under the given frontend key.
 * @param {string} key - frontend-owned routing key (usually a UUID)
 * @param {string} cwd
 * @param {string|null} sessionId - optional session to resume
 * @returns {Promise<{ key: string, sessionId: string, reattached: boolean }>}
 */
export async function startZeroSession(key, cwd, sessionId = null) {
  return invoke("start_zero_session", { key, cwd, sessionId });
}

/**
 * Send a user message to a specific session.
 * @param {string} key
 * @param {string} content
 * @param {{ mimeType: string, data: string, name: string } | null} [file]
 */
export async function sendZeroMessage(key, content, file = null) {
  return invoke("send_zero_message", { key, content, file });
}

/**
 * Read a file picked from the native file dialog and return it
 * base64-encoded, ready to preview or attach to a message.
 * @param {string} path - absolute path to the file
 * @returns {Promise<{ mimeType: string, data: string, name: string }>}
 */
export async function readFileAttachment(path) {
  return invoke("read_file_attachment", { path });
}

/**
 * List one directory level (folders + files), for the file explorer tree.
 * Not recursive - children are loaded lazily as folders are expanded.
 * @param {string} path - absolute directory path
 * @returns {Promise<Array<{ name: string, path: string, isDir: boolean }>>}
 */
export async function listDirectoryEntries(path) {
  return invoke("list_directory_entries", { path });
}

/**
 * Stop a specific session and remove its record.
 * @param {string} key
 */
export async function stopZeroSession(key) {
  return invoke("stop_zero_session", { key });
}

/**
 * Cancel the in-flight turn for a specific session without tearing it down.
 * @param {string} key
 */
export async function cancelZeroRun(key) {
  return invoke("cancel_zero_run", { key });
}

/**
 * List the active provider's live model list plus which one is active.
 * @returns {Promise<{ models: string[], active: string, capabilities: Record<string, { reasoning: boolean, reasoningEfforts: string[] }> }>}
 */
export async function listZeroModels() {
  return invoke("list_zero_models");
}

/**
 * Switch the active provider's model and restart only the session in focus.
 * @param {string} key - session key currently focused
 * @param {string} model - id as returned by listZeroModels()
 */
export async function switchZeroModel(key, model) {
  return invoke("switch_zero_model", { key, model });
}

/**
 * Persist a session's model choice by session id directly, without
 * requiring a live connection - for a panel that hasn't (re)connected yet.
 * Picked up automatically the next time this session's process spawns.
 * @param {string} sessionId
 * @param {string} model
 */
export async function setSessionModelById(sessionId, model) {
  return invoke("set_zero_session_model_by_id", { sessionId, model });
}

/**
 * Switch this session's reasoning-effort preference. Live, no restart.
 * @param {string} key - session key currently focused
 * @param {string} effort - "" for auto, or a modelregistry ReasoningEffort value
 */
export async function switchZeroEffort(key, effort) {
  return invoke("switch_zero_effort", { key, effort });
}

/**
 * Persist a session's reasoning-effort choice by session id directly,
 * without requiring a live connection - mirrors setSessionModelById.
 * @param {string} sessionId
 * @param {string} effort
 */
export async function setSessionEffortById(sessionId, effort) {
  return invoke("set_zero_session_effort_by_id", { sessionId, effort });
}

/**
 * Switch this session's ACP permission mode ("auto" | "ask" | "spec-draft")
 * - the ACP-native equivalent of Claude Code's Plan Mode. Live, no restart.
 * Requires the session to be connected; use `setSessionModeById` otherwise.
 * @param {string} key - session key currently focused
 * @param {"auto" | "ask" | "spec-draft"} mode
 */
export async function setSessionMode(key, mode) {
  return invoke("switch_zero_mode", { key, mode });
}

/**
 * Persist a session's mode by session id directly, without requiring a live
 * connection - for a panel that hasn't (re)connected yet. Picked up
 * automatically the next time this session's process spawns.
 * @param {string} sessionId
 * @param {"auto" | "ask" | "spec-draft"} mode
 */
export async function setSessionModeById(sessionId, mode) {
  return invoke("set_zero_session_mode_by_id", { sessionId, mode });
}

/**
 * Read a session's persisted plan state (mode + an eventual pending spec
 * awaiting review). Pure disk read, no live session required - used to
 * restore the Plan Mode toggle and review dialog on session recovery.
 * @param {string} sessionId
 * @returns {Promise<{ mode: string, pendingSpec: { specId: string, title: string, filePath: string, relativePath: string } | null } | null>}
 */
export async function getSessionPlanState(sessionId) {
  return invoke("get_zero_session_plan_state", { sessionId });
}

/**
 * Clear a session's pending spec review (after it's been approved or the
 * user requested changes).
 * @param {string} sessionId
 */
export async function clearPendingSpec(sessionId) {
  return invoke("clear_zero_pending_spec", { sessionId });
}

/**
 * Read a spec markdown file's content (the `filePath` from a
 * `spec_review_required` event or a persisted `pendingSpec`) for the
 * plan-review dialog.
 * @param {string} path - absolute path
 * @returns {Promise<string>}
 */
export async function readSpecFile(path) {
  return invoke("read_spec_file", { path });
}

/**
 * Listen for zero stream-json events. The callback receives the raw event;
 * consumers must filter by `payload.sessionKey`.
 * @param {(event: { payload: any }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onZeroEvent(callback) {
  return listen("zero:event", callback);
}

/**
 * Listen for zero stderr lines. Consumers must filter by `payload.sessionKey`.
 * @param {(event: { payload: { sessionKey: string, line: string } }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onZeroStderr(callback) {
  return listen("zero:stderr", callback);
}

/**
 * Listen for the zero process exiting. Consumers must filter by `payload.sessionKey`.
 * @param {(event: { payload: { sessionKey: string } }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onZeroProcessExited(callback) {
  return listen("zero:process-exited", callback);
}

/**
 * List sessions from zero for the given workspace.
 * @param {string} cwd
 * @returns {Promise<Array<{ session_id: string, title: string, created_at: string, cwd: string, model_id: string }>>}
 */
export async function listZeroSessions(cwd) {
  return invoke("list_zero_sessions", { cwd });
}

/**
 * Load message history for a specific session.
 * @param {string} sessionId
 * @returns {Promise<Array<{ role: string, content: string, timestamp: string }>>}
 */
export async function loadSessionHistory(sessionId) {
  return invoke("load_session_history", { sessionId });
}

/**
 * Delete a session from disk.
 * @param {string} sessionId
 */
export async function deleteSession(sessionId) {
  return invoke("delete_session", { sessionId });
}

/**
 * Rename a session.
 * @param {string} sessionId
 * @param {string} title
 */
export async function renameSession(sessionId, title) {
  return invoke("rename_session", { sessionId, title });
}

/**
 * Answer a pending permission request from zero.
 * @param {string} requestId - correlation id from the permission-request event
 * @param {string} optionId - one of the option ids offered in that event
 */
export async function respondToPermission(requestId, optionId) {
  return invoke("respond_to_permission", { requestId, optionId });
}

/**
 * Listen for a real permission request from zero.
 * Consumers must filter by `payload.sessionKey`.
 * @param {(event: { payload: { requestId: string, toolName: string, reason: string, options: Array, sessionKey: string } }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onZeroPermissionRequest(callback) {
  return listen("zero:permission-request", callback);
}

/**
 * List sessions currently tracked by the bridge, with live status.
 * @returns {Promise<Array<{ key: string, sessionId: string, cwd: string, live: boolean }>>}
 */
export async function listLiveSessions() {
  return invoke("list_live_sessions");
}

/**
 * List configured MCP and hook backends from zero's config.
 * @returns {Promise<Array<{ name: string, type: string, url: string|null, toolCount: number, headerCount: number }>>}
 */
export async function listMcpBackends() {
  return invoke("list_mcp_backends");
}

/**
 * Live-check a single MCP server.
 * @param {string} name
 * @returns {Promise<{ serverName: string, status: string, toolCount: number, tools: Array }>}
 */
export async function checkMcpBackend(name) {
  return invoke("check_mcp_backend", { name });
}

/**
 * List all tools exposed by enabled MCP servers.
 * @returns {Promise<Array<{ name: string, description: string | null }>>}
 */
export async function listMcpTools() {
  return invoke("list_mcp_tools");
}

/**
 * Load the persisted MCP status cache from disk.
 * @returns {Promise<{ servers: Record<string, { status: string, toolCount: number, error: string|null, checkedAt: number|null }>, generatedAt: number|null }>}
 */
export async function loadMcpStatusCache() {
  return invoke("load_mcp_status_cache");
}

/**
 * Get the global advisor configuration.
 * @returns {Promise<{ enabled: boolean, model: string | null }>}
 */
export async function getAdvisorConfig() {
  return invoke("get_advisor_config");
}

/**
 * Save the global advisor configuration.
 * @param {{ enabled: boolean, model: string | null }} config
 */
export async function setAdvisorConfig(config) {
  return invoke("set_advisor_config", { config });
}

/**
 * Toggle the advisor enabled state globally.
 * @param {boolean} enabled
 * @returns {Promise<{ enabled: boolean, model: string | null }>}
 */
export async function toggleAdvisor(enabled) {
  return invoke("toggle_advisor", { enabled });
}

/**
 * Set the advisor model globally.
 * @param {string | null} model
 * @returns {Promise<{ enabled: boolean, model: string | null }>}
 */
export async function setAdvisorModel(model) {
  return invoke("set_advisor_model", { model });
}

/**
 * Get the advisor configuration for a specific session.
 * @param {string} key - session key
 * @returns {Promise<{ enabled: boolean, model: string | null }>}
 */
export async function getSessionAdvisorConfig(key) {
  return invoke("get_session_advisor_config", { key });
}

/**
 * Set the advisor configuration for a specific session.
 * @param {string} key - session key
 * @param {{ enabled: boolean, model: string | null }} config
 */
export async function setSessionAdvisorConfig(key, config) {
  return invoke("set_session_advisor_config", { key, config });
}
