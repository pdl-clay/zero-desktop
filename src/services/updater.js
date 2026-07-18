import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// Holds the real Update instance between checkForUpdate() and
// downloadAndInstallUpdate() - kept out of Pinia state (which wraps
// everything in a reactive proxy that a class instance with methods
// doesn't survive) between the two calls.
let pendingUpdate = null;

/**
 * @returns {Promise<string>} the running app's own version (tauri.conf.json's `version`).
 */
export async function getAppVersion() {
  return getVersion();
}

/**
 * @returns {Promise<boolean>} whether this process is running as the packaged
 * AppImage (vs `tauri dev` or a bare binary) - self-update only applies then.
 */
export async function isAppImageRuntime() {
  return invoke("is_appimage");
}

/**
 * Check the configured update endpoint for a newer version.
 * @returns {Promise<{ version: string, notes: string|null, date: string|null } | null>} null if already up to date.
 */
export async function checkForUpdate() {
  pendingUpdate = await check();
  if (!pendingUpdate) return null;
  return {
    version: pendingUpdate.version,
    notes: pendingUpdate.body ?? null,
    date: pendingUpdate.date ?? null,
  };
}

/**
 * Download and install the update found by the last checkForUpdate() call.
 * Does NOT restart the app - call restartToApplyUpdate() separately once
 * the user confirms.
 * @param {(progress: { downloaded: number, total: number }) => void} [onProgress]
 */
export async function downloadAndInstallUpdate(onProgress) {
  if (!pendingUpdate) throw new Error("No pending update - call checkForUpdate() first");
  let downloaded = 0;
  let total = 0;
  await pendingUpdate.downloadAndInstall((event) => {
    if (event.event === "Started") total = event.data.contentLength ?? 0;
    else if (event.event === "Progress") downloaded += event.data.chunkLength;
    onProgress?.({ downloaded, total });
  });
}

/**
 * Restart the app to run the just-installed update.
 */
export async function restartToApplyUpdate() {
  return relaunch();
}
