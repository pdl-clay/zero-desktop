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
 * Send a user message, with an optional image attachment, to the active
 * zero session.
 * @param {string} content
 * @param {{ mimeType: string, data: string, name: string } | null} [image]
 */
export async function sendZeroMessage(content, image = null) {
  return invoke("send_zero_message", { content, image });
}

/**
 * Read an image file picked from the native file dialog and return it
 * base64-encoded, ready to preview (as a `data:` URI) or attach to a
 * message. Rejects files over 10MB and unsupported extensions server-side.
 * @param {string} path - absolute path to the image file
 * @returns {Promise<{ mimeType: string, data: string, name: string }>}
 */
export async function readImageAttachment(path) {
  return invoke("read_image_attachment", { path });
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
