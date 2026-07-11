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
 * Send a user message to the active zero session.
 * @param {string} content
 */
export async function sendZeroMessage(content) {
  return invoke("send_zero_message", { content });
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
 * Send a permission decision back to zero.
 * @param {string} permissionId
 * @param {string} decision - "approved" or "denied"
 */
export async function sendPermissionDecision(permissionId, decision) {
  return invoke("send_permission_decision", { permissionId, decision });
}
