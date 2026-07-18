/**
 * Simulated store behavior tests for the zero-desktop self-updater.
 * Run: node tests/updater-store.test.js
 *
 * zero-store.js can't be imported directly here - it pulls in Pinia and
 * @tauri-apps/plugin-updater/@tauri-apps/plugin-process through the
 * `@/services/updater` import chain, none of which run under plain `node`.
 * This mirrors checkForUpdates/downloadAndInstallUpdate/restartToApplyUpdate's
 * actual bodies (see src/stores/zero-store.js) with injected fake service
 * functions instead of the real Tauri plugin calls.
 */

let passed = 0;
let failed = 0;

function assert(condition, name) {
  if (condition) {
    passed++;
    console.log(`  ✓ ${name}`);
  } else {
    failed++;
    console.log(`  ✗ ${name}`);
  }
}

function assertEquals(actual, expected, name) {
  assert(
    JSON.stringify(actual) === JSON.stringify(expected),
    `${name} (got ${JSON.stringify(actual)})`,
  );
}

// Mirrors zero-store.js's update-related state + actions, with the
// services/updater.js calls swapped for injected fakes.
function createUpdaterStore({ checkForUpdate, downloadAndInstallUpdate, restartToApplyUpdate }) {
  return {
    isAppImageRuntime: true,
    updateAvailable: false,
    updateInfo: null,
    isCheckingUpdate: false,
    isDownloadingUpdate: false,
    downloadProgress: { downloaded: 0, total: 0 },
    updateReadyToInstall: false,
    updateError: null,

    async checkForUpdates({ silent = false } = {}) {
      if (!this.isAppImageRuntime || this.isCheckingUpdate) return;
      this.isCheckingUpdate = true;
      this.updateError = null;
      try {
        const info = await checkForUpdate();
        this.updateAvailable = Boolean(info);
        this.updateInfo = info;
      } catch (error) {
        this.updateError = error;
        if (!silent) throw error;
      } finally {
        this.isCheckingUpdate = false;
      }
    },

    async downloadAndInstallUpdate() {
      if (!this.updateAvailable || this.isDownloadingUpdate) return;
      this.isDownloadingUpdate = true;
      this.downloadProgress = { downloaded: 0, total: 0 };
      try {
        await downloadAndInstallUpdate((progress) => {
          this.downloadProgress = progress;
        });
        this.updateReadyToInstall = true;
      } catch (error) {
        this.updateError = error;
        throw error;
      } finally {
        this.isDownloadingUpdate = false;
      }
    },

    async restartToApplyUpdate() {
      await restartToApplyUpdate();
    },
  };
}

console.log("Updater store behavior tests\n");

// --- checkForUpdates: found vs. not found ---
console.log("checkForUpdates:");

const hit = createUpdaterStore({
  checkForUpdate: async () => ({ version: "0.2.0", notes: "fixes", date: "2026-07-17" }),
  downloadAndInstallUpdate: async () => {},
  restartToApplyUpdate: async () => {},
});
await hit.checkForUpdates();
assertEquals(hit.updateAvailable, true, "sets updateAvailable when a newer version is found");
assertEquals(hit.updateInfo.version, "0.2.0", "stores the update info");
assertEquals(hit.isCheckingUpdate, false, "isCheckingUpdate resets to false after the check");

const miss = createUpdaterStore({
  checkForUpdate: async () => null,
  downloadAndInstallUpdate: async () => {},
  restartToApplyUpdate: async () => {},
});
await miss.checkForUpdates();
assertEquals(miss.updateAvailable, false, "leaves updateAvailable false when already up to date");
assertEquals(miss.updateInfo, null, "leaves updateInfo null when already up to date");

// --- checkForUpdates: isCheckingUpdate toggles around the call ---
console.log("\nisCheckingUpdate toggling:");

let sawCheckingDuringCall = false;
const toggling = createUpdaterStore({
  checkForUpdate: async () => {
    sawCheckingDuringCall = toggling.isCheckingUpdate;
    return null;
  },
  downloadAndInstallUpdate: async () => {},
  restartToApplyUpdate: async () => {},
});
assertEquals(toggling.isCheckingUpdate, false, "starts false");
await toggling.checkForUpdates();
assert(sawCheckingDuringCall, "isCheckingUpdate is true while the check is in flight");
assertEquals(toggling.isCheckingUpdate, false, "isCheckingUpdate is false again once settled");

// --- checkForUpdates: silent vs. non-silent error handling ---
console.log("\nsilent vs. non-silent error handling:");

const silentFailure = createUpdaterStore({
  checkForUpdate: async () => {
    throw new Error("network down");
  },
  downloadAndInstallUpdate: async () => {},
  restartToApplyUpdate: async () => {},
});
await silentFailure.checkForUpdates({ silent: true });
assert(silentFailure.updateError !== null, "silent mode still records the error");
assertEquals(silentFailure.isCheckingUpdate, false, "silent mode still resets isCheckingUpdate");

const loudFailure = createUpdaterStore({
  checkForUpdate: async () => {
    throw new Error("network down");
  },
  downloadAndInstallUpdate: async () => {},
  restartToApplyUpdate: async () => {},
});
let threw = false;
try {
  await loudFailure.checkForUpdates({ silent: false });
} catch {
  threw = true;
}
assert(threw, "non-silent mode rethrows the error");

// --- downloadAndInstallUpdate ---
console.log("\ndownloadAndInstallUpdate:");

const noUpdateYet = createUpdaterStore({
  checkForUpdate: async () => null,
  downloadAndInstallUpdate: async () => {
    throw new Error("should not be called without a pending update");
  },
  restartToApplyUpdate: async () => {},
});
await noUpdateYet.downloadAndInstallUpdate();
assertEquals(
  noUpdateYet.updateReadyToInstall,
  false,
  "does nothing when there is no update available",
);

const download = createUpdaterStore({
  checkForUpdate: async () => ({ version: "0.2.0", notes: null, date: null }),
  downloadAndInstallUpdate: async (onProgress) => {
    onProgress({ downloaded: 50, total: 100 });
    onProgress({ downloaded: 100, total: 100 });
  },
  restartToApplyUpdate: async () => {},
});
await download.checkForUpdates();
assertEquals(download.updateReadyToInstall, false, "not ready to install before downloading");
await download.downloadAndInstallUpdate();
assertEquals(
  download.updateReadyToInstall,
  true,
  "updateReadyToInstall becomes true only after a successful download",
);
assertEquals(
  download.downloadProgress,
  { downloaded: 100, total: 100 },
  "tracks download progress",
);
assertEquals(download.isDownloadingUpdate, false, "isDownloadingUpdate resets to false once done");

const downloadFailure = createUpdaterStore({
  checkForUpdate: async () => ({ version: "0.2.0", notes: null, date: null }),
  downloadAndInstallUpdate: async () => {
    throw new Error("disk full");
  },
  restartToApplyUpdate: async () => {},
});
await downloadFailure.checkForUpdates();
let downloadThrew = false;
try {
  await downloadFailure.downloadAndInstallUpdate();
} catch {
  downloadThrew = true;
}
assert(downloadThrew, "downloadAndInstallUpdate rethrows on failure");
assertEquals(
  downloadFailure.updateReadyToInstall,
  false,
  "a failed download never marks updateReadyToInstall",
);

// --- restartToApplyUpdate ---
console.log("\nrestartToApplyUpdate:");

let relaunchCalls = 0;
const restart = createUpdaterStore({
  checkForUpdate: async () => null,
  downloadAndInstallUpdate: async () => {},
  restartToApplyUpdate: async () => {
    relaunchCalls++;
  },
});
await restart.restartToApplyUpdate();
assertEquals(relaunchCalls, 1, "delegates to the relaunch service exactly once");

// --- Summary ---
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed > 0 ? 1 : 0);
