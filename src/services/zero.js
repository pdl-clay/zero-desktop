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
 * Start a new zero exec session in the given workspace.
 * @param {string} cwd
 * @param {string|null} sessionId - optional session to resume
 */
export async function startZeroSession(cwd, sessionId = null) {
  return invoke("start_zero_session", { cwd, sessionId });
}

/**
 * Send a user message, with an optional file attachment, to the active
 * zero session.
 * @param {string} content
 * @param {{ mimeType: string, data: string, name: string } | null} [file]
 */
export async function sendZeroMessage(content, file = null) {
  return invoke("send_zero_message", { content, file });
}

/**
 * Read a file picked from the native file dialog and return it
 * base64-encoded, ready to preview or attach to a message. Rejects files
 * over 10MB and unsupported extensions server-side.
 * @param {string} path - absolute path to the file
 * @returns {Promise<{ mimeType: string, data: string, name: string }>}
 */
export async function readFileAttachment(path) {
  return invoke("read_file_attachment", { path });
}

/**
 * Stop the active zero session.
 */
export async function stopZeroSession() {
  return invoke("stop_zero_session");
}

/**
 * Cancel the in-flight turn without tearing down the session, so the next
 * message can still resume the same zero session.
 */
export async function cancelZeroRun() {
  return invoke("cancel_zero_run");
}

/**
 * List the active provider's live model list plus which one is active. A
 * real network probe against the provider's own model-listing endpoint (per
 * zero's own docs) - call on demand, not on every session start.
 * @returns {Promise<{ models: string[], active: string }>}
 */
export async function listZeroModels() {
  return invoke("list_zero_models");
}

/**
 * Switch the active provider's model. A global, persisted zero CLI/config
 * change, not a per-session one - the ACP transport has no method for this
 * (verified live: session/set_model and session/models both return "method
 * not found"), so it affects every zero process on this machine. Kills the
 * current live zero acp process so the next message respawns under the new
 * model; session id and history are preserved via the existing
 * session/load reattach path.
 * @param {string} model - id as returned by listZeroModels()
 */
export async function switchZeroModel(model) {
  return invoke("switch_zero_model", { model });
}

/**
 * Listen for zero stream-json events.
 * @param {(event: { event: string, payload: any }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onZeroEvent(callback) {
  return listen("zero:event", callback);
}

/**
 * Listen for zero stderr lines.
 * @param {(event: { payload: string }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onZeroStderr(callback) {
  return listen("zero:stderr", callback);
}

/**
 * Listen for the zero process exiting (stdout stream closed).
 * @param {(event: { payload: null }) => void} callback
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
 * Rename a session. zero itself gives ACP-created sessions a generic "ACP
 * session" title with no protocol method found to change it, so
 * zero-desktop tracks titles locally (auto-set from the first message, or
 * overridden here).
 * @param {string} sessionId
 * @param {string} title
 */
export async function renameSession(sessionId, title) {
  return invoke("rename_session", { sessionId, title });
}

/**
 * Answer a pending permission request from zero. Unlike the old exec-based
 * transport, this reply actually reaches the agent (delivered over the
 * persistent zero acp JSON-RPC connection).
 * @param {string} requestId - correlation id from the permission-request event
 * @param {string} optionId - one of the option ids offered in that event
 */
export async function respondToPermission(requestId, optionId) {
  return invoke("respond_to_permission", { requestId, optionId });
}

/**
 * Listen for a real permission request from zero, awaiting a reply.
 * @param {(event: { payload: { requestId: string, toolName: string, reason: string, options: Array } }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onZeroPermissionRequest(callback) {
  return listen("zero:permission-request", callback);
}

/**
 * List configured MCP and hook backends from zero's config.
 * Fast, read-only — no connections attempted.
 * @returns {Promise<Array<{ name: string, type: string, url: string|null, toolCount: number, headerCount: number }>>}
 */
export async function listMcpBackends() {
  return invoke("list_mcp_backends");
}

/**
 * Live-check a single MCP server: connects, lists tools, reports status.
 * @param {string} name - server name as in zero config
 * @returns {Promise<{ serverName: string, status: string, toolCount: number, tools: Array }>}
 */
export async function checkMcpBackend(name) {
  return invoke("check_mcp_backend", { name });
}

/**
 * List all tools exposed by enabled MCP servers.
 * Unlike `checkMcpBackend`, this returns real tools with descriptions.
 * @returns {Promise<Array<{ name: string, description: string | null }>>}
 */
export async function listMcpTools() {
  return invoke("list_mcp_tools");
}

/**
 * Load the persisted MCP status cache from disk.
 * Returns immediately with cached statuses so the drawer can render
 * without waiting for live checks.
 * @returns {Promise<{ servers: Record<string, { status: string, toolCount: number, error: string|null, checkedAt: number|null }>, generatedAt: number|null }>}
 */
export async function loadMcpStatusCache() {
  return invoke("load_mcp_status_cache");
}
