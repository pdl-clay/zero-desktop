/**
 * Simulated store behavior tests for Plan Mode's session-mode persistence.
 * Run: node tests/plan-mode-store.test.js
 *
 * Regression coverage for a real bug: picking Plan Mode ("spec-draft") on a
 * brand-new panel (no sessionId yet) only marks the choice dirty in-memory
 * (setMode has nowhere to persist to before a session exists - see
 * zero-session-store.js's setMode/_sessionModeDirty). sendMessage's
 * lazy-connect then calls startSession(cwd, null), whose "reset state for a
 * genuinely new session" block used to unconditionally set
 * `this.sessionMode = "auto"` - clobbering the dirty "spec-draft" choice
 * BEFORE _syncPlanStateFromDisk got a chance to flush it, so "auto" (not
 * "spec-draft") was what actually got pushed to the engine. Symptom: the
 * mode picker visibly flips back to Auto right after sending the first
 * message, and the model never receives the read-only/plan-drafting system
 * prompt because the engine was never actually told to switch out of auto.
 *
 * zero-session-store.js can't be imported directly under plain `node` (it
 * pulls in Pinia/Vue/@tauri-apps/api/core) - this mirrors startSession's
 * reset block and _syncPlanStateFromDisk's flush exactly as they're
 * implemented there.
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
    `${name} (got ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)})`,
  );
}

// Mirrors zero-session-store.js's sessionMode-related state and actions.
function createSessionModeStore() {
  return {
    sessionId: null,
    isConnected: false,
    sessionMode: "auto",
    _sessionModeDirty: false,
    pendingPlanReview: { specId: "stale" }, // any pre-existing value; must be cleared on a fresh session
    backendCalls: [],

    // Mirrors setMode(mode)
    setMode(mode) {
      this.sessionMode = mode;
      if (this.isConnected) {
        this.backendCalls.push({ via: "live", mode });
      } else if (this.sessionId) {
        this.backendCalls.push({ via: "byId", mode });
      } else {
        this._sessionModeDirty = true;
      }
    },

    // Mirrors startSession(cwd, sessionId)'s reset block + connect + the
    // _syncPlanStateFromDisk call at the end of its success path.
    async startSession(cwd, sessionId = null) {
      const alreadyPrepared = Boolean(sessionId) && this.sessionId === sessionId;
      if (!alreadyPrepared) {
        if (!this._sessionModeDirty) {
          this.sessionMode = "auto";
        }
        this.pendingPlanReview = null;
      }
      this.sessionId = sessionId;
      // Simulates startZeroSession resolving with the real backend id.
      this.sessionId = this.sessionId || "new-session-id";
      this.isConnected = true;
      await this._syncPlanStateFromDisk();
    },

    // Mirrors _syncPlanStateFromDisk()
    async _syncPlanStateFromDisk() {
      if (!this.sessionId) return;
      if (this._sessionModeDirty) {
        this._sessionModeDirty = false;
        this.backendCalls.push({ via: "flush", mode: this.sessionMode });
        return;
      }
      // (disk restore path not needed for these tests)
    },
  };
}

console.log("Plan Mode store behavior tests\n");

// --- The regression itself ---
console.log("setMode(\"spec-draft\") on a brand-new panel, then first sendMessage:");

const freshPanel = createSessionModeStore();
freshPanel.setMode("spec-draft");
assertEquals(freshPanel.sessionMode, "spec-draft", "selecting Plan Mode updates local state immediately");
assert(freshPanel._sessionModeDirty, "marks the choice dirty (no sessionId to persist to yet)");
assertEquals(freshPanel.backendCalls.length, 0, "does not call the backend before a session exists");

await freshPanel.startSession("/workspace", null);
assertEquals(
  freshPanel.sessionMode,
  "spec-draft",
  "Plan Mode survives the lazy-connect reset instead of being stomped back to auto",
);
assert(!freshPanel._sessionModeDirty, "dirty flag is cleared once flushed");
assertEquals(
  freshPanel.backendCalls,
  [{ via: "flush", mode: "spec-draft" }],
  "the engine actually receives spec-draft, not auto",
);
assertEquals(freshPanel.pendingPlanReview, null, "a genuinely new session still clears any stale pending review");

// --- Reconnecting to an already-known session must not reset the mode ---
console.log("\nreconnecting to an already-prepared session (same sessionId):");

const resumed = createSessionModeStore();
resumed.sessionId = "existing-id";
resumed.sessionMode = "ask";
await resumed.startSession("/workspace", "existing-id");
assertEquals(resumed.sessionMode, "ask", "an already-prepared reconnect keeps whatever mode was already set");

// --- A genuinely new session with no mode opinion still defaults to auto ---
console.log("\nbrand-new panel with no mode ever selected:");

const plainNewPanel = createSessionModeStore();
await plainNewPanel.startSession("/workspace", null);
assertEquals(plainNewPanel.sessionMode, "auto", "no dirty choice means the reset-to-auto still applies as before");
assertEquals(plainNewPanel.backendCalls.length, 0, "nothing dirty to flush, so no backend call is made");

// --- Picking "ask" (not just Plan Mode) on a brand-new panel is covered too ---
console.log("\nsetMode(\"ask\") on a brand-new panel:");

const askPanel = createSessionModeStore();
askPanel.setMode("ask");
await askPanel.startSession("/workspace", null);
assertEquals(askPanel.sessionMode, "ask", "any non-auto dirty mode survives the lazy-connect reset, not just spec-draft");
assertEquals(askPanel.backendCalls, [{ via: "flush", mode: "ask" }], "the engine receives ask, not auto");

// --- Summary ---
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed > 0 ? 1 : 0);
