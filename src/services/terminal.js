import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Spawn a new PTY-backed shell under the given frontend key.
 * @param {string} key - frontend-owned routing key (usually a UUID)
 * @param {string} cwd
 * @param {number} cols
 * @param {number} rows
 * @returns {Promise<{ key: string, pid: number | null, shell: string }>}
 */
export async function spawnTerminal(key, cwd, cols, rows) {
  return invoke("spawn_terminal", { key, cwd, cols, rows });
}

/**
 * Write raw input (keystrokes, pasted text) to a terminal's stdin.
 * @param {string} key
 * @param {string} data
 */
export async function writeTerminal(key, data) {
  return invoke("write_terminal", { key, data });
}

/**
 * Tell the pty its viewport has resized, so the shell/apps inside it can
 * reflow (e.g. `$COLUMNS`, curses UIs).
 * @param {string} key
 * @param {number} cols
 * @param {number} rows
 */
export async function resizeTerminal(key, cols, rows) {
  return invoke("resize_terminal", { key, cols, rows });
}

/**
 * Kill a terminal's shell process outright.
 * @param {string} key
 */
export async function killTerminal(key) {
  return invoke("kill_terminal", { key });
}

/**
 * List terminals currently tracked by the backend, for reconciliation.
 * @returns {Promise<Array<{ key: string, cwd: string, live: boolean }>>}
 */
export async function listTerminals() {
  return invoke("list_terminals");
}

/**
 * Listen for pty output. Consumers must filter by `payload.key`.
 * @param {(event: { payload: { key: string, data: string } }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onTerminalData(callback) {
  return listen("terminal:data", callback);
}

/**
 * Listen for a terminal's shell process exiting. Consumers must filter by
 * `payload.key`.
 * @param {(event: { payload: { key: string, exitCode: number | null } }) => void} callback
 * @returns {Promise<() => void>}
 */
export async function onTerminalExit(callback) {
  return listen("terminal:exit", callback);
}
