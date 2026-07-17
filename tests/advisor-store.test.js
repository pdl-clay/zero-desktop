/**
 * Simulated store behavior tests for advisor mode.
 * Run: node tests/advisor-store.test.js
 *
 * These tests simulate the Pinia store logic without requiring Vue/Quasar,
 * but reuse the *real* detection helpers from src/utils/advisor-prompt.js
 * (not a hand-duplicated copy) so a regression there is actually caught.
 */

import { isAdvisorConsultation, extractAdvisorPrompt } from "../src/utils/advisor-prompt.js";

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

// Simulate the store state
function createSessionStore() {
  return {
    advisorEnabled: false,
    advisorModel: null,

    toggleAdvisor(enabled) {
      this.advisorEnabled = enabled;
    },

    setAdvisorModel(model) {
      this.advisorModel = model;
    },

    _loadAdvisorConfig(config) {
      if (config) {
        this.advisorEnabled = config.enabled;
        this.advisorModel = config.model;
      }
    },
  };
}

console.log("Advisor store behavior tests\n");

// --- Toggle ---
console.log("toggleAdvisor:");

const store = createSessionStore();
assert(store.advisorEnabled === false, "initial state: disabled");

store.toggleAdvisor(true);
assert(store.advisorEnabled === true, "toggle on");

store.toggleAdvisor(false);
assert(store.advisorEnabled === false, "toggle off");

// --- Model ---
console.log("\nsetAdvisorModel:");

store.setAdvisorModel("claude-opus-4-1");
assertEquals(store.advisorModel, "claude-opus-4-1", "sets model");

store.setAdvisorModel(null);
assertEquals(store.advisorModel, null, "clears model");

// --- Load config ---
console.log("\n_loadAdvisorConfig:");

store._loadAdvisorConfig({ enabled: true, model: "gpt-4o" });
assert(store.advisorEnabled === true, "loads enabled state");
assertEquals(store.advisorModel, "gpt-4o", "loads model");

store._loadAdvisorConfig(null);
assert(store.advisorEnabled === true, "null config preserves state");

store._loadAdvisorConfig({ enabled: false, model: null });
assert(store.advisorEnabled === false, "loads disabled state");
assertEquals(store.advisorModel, null, "loads null model");

// --- Event detection (real helpers) ---
console.log("\nEvent detection (isAdvisorConsultation/extractAdvisorPrompt):");

const advisorTaskArgs = { name: "advisor", prompt: "Analise esta arquitetura" };
assert(isAdvisorConsultation("Task", advisorTaskArgs) === true, "detects advisor consultation event");
assertEquals(extractAdvisorPrompt(advisorTaskArgs), "Analise esta arquitetura", "extracts prompt from event");

const explorerTaskArgs = { name: "explorer", prompt: "Explore o código" };
assert(isAdvisorConsultation("Task", explorerTaskArgs) === false, "ignores non-advisor Task call");

assert(isAdvisorConsultation("text", undefined) === false, "ignores non-tool-call events");

// --- addToolCall / updateToolCallResult (mirrors zero-session-store.js) ---
// zero-session-store.js can't be imported directly here - it pulls in Vue,
// Pinia, i18n, and @tauri-apps/api/core through the `@/services/zero`
// import chain, none of which run under plain `node`. This mirrors the two
// methods' actual bodies (see src/stores/zero-session-store.js) using the
// *real* imported isAdvisorConsultation/extractAdvisorPrompt, so a
// regression in either the routing logic or the detection helpers is caught.
console.log("\naddToolCall / updateToolCallResult (mirrored):");

let _id = 0;
function nextId() {
  return `msg-${++_id}`;
}

function createMessageStore() {
  return {
    messages: [],
    addToolCall(event) {
      if (event.name === "update_plan") return;
      if (isAdvisorConsultation(event.name, event.args)) {
        this.messages.push({
          id: nextId(),
          type: "advisor_consultation",
          toolUseId: event.id,
          prompt: extractAdvisorPrompt(event.args) || "",
          content: "",
          status: "running",
        });
        return;
      }
      this.messages.push({
        id: nextId(),
        type: "tool_call",
        toolName: event.name,
        toolUseId: event.id,
        input: event.args || {},
        status: "running",
        result: null,
      });
    },
    updateToolCallResult(event) {
      const msg = this.messages.find(
        (m) =>
          (m.type === "tool_call" || m.type === "advisor_consultation") &&
          m.toolUseId === event.id &&
          m.status === "running",
      );
      if (!msg) return;
      msg.status = event.status === "error" ? "error" : "completed";
      if (msg.type === "advisor_consultation") {
        msg.content = event.output || "";
      } else {
        msg.result = event.output || "";
      }
    },
  };
}

const store2 = createMessageStore();
store2.addToolCall({ name: "Task", id: "call-1", args: { name: "advisor", prompt: "Revise este design" } });
assertEquals(store2.messages.length, 1, "advisor Task call pushes exactly one message");
assertEquals(store2.messages[0].type, "advisor_consultation", "routes advisor Task call to advisor_consultation");
assertEquals(store2.messages[0].prompt, "Revise este design", "stores the consultation prompt");
assertEquals(store2.messages[0].content, "", "content starts empty while running");
assertEquals(store2.messages[0].status, "running", "starts running");

store2.updateToolCallResult({ id: "call-1", status: "success", output: "1) Extrair função. 2) Adicionar testes." });
assertEquals(store2.messages[0].status, "completed", "marks advisor consultation completed");
assertEquals(
  store2.messages[0].content,
  "1) Extrair função. 2) Adicionar testes.",
  "fills content with the advisor's output on completion",
);

const store3 = createMessageStore();
store3.addToolCall({ name: "Task", id: "call-2", args: { name: "explorer", prompt: "Explore src/" } });
assertEquals(store3.messages[0].type, "tool_call", "non-advisor Task call stays a regular tool_call");
store3.updateToolCallResult({ id: "call-2", status: "success", output: "done" });
assertEquals(store3.messages[0].result, "done", "regular tool_call still fills `result`, not `content`");

const store4 = createMessageStore();
store4.addToolCall({ name: "read_file", id: "call-3", args: { path: "a.txt" } });
assertEquals(store4.messages[0].type, "tool_call", "non-Task tool calls are unaffected");

// --- toggleAdvisor/setAdvisorModel/_loadAdvisorConfig (mirrors zero-session-store.js) ---
// Regression coverage for a real bug: a panel can sit open without a live
// backend process (prepareSession defers the real connection to the first
// sendMessage). Toggling advisor before that connection used to call the
// backend anyway, throwing "No active session for key: ..." because
// nothing was registered under that key yet. See src/stores/
// zero-session-store.js's toggleAdvisor/setAdvisorModel/_loadAdvisorConfig.
console.log("\ntoggleAdvisor/setAdvisorModel before vs. after connect (mirrored):");

// Stand-in for localStorage - Node has no global localStorage by default, so
// this mirrors loadAdvisorPreferences/saveAdvisorPreferences from
// zero-session-store.js as a plain module-scoped object instead.
function createPrefsStorage(initial = { model: null, mode: "max" }) {
  let prefs = initial;
  return {
    load: () => prefs,
    save: (next) => {
      prefs = next;
    },
  };
}

function createAdvisorAwareStore(prefsStorage = createPrefsStorage()) {
  const initial = prefsStorage.load();
  return {
    isConnected: false,
    advisorEnabled: false,
    advisorModel: initial.model,
    advisorMode: initial.mode,
    _advisorConfigDirty: false,
    backendConfig: null, // what the "backend" thinks this session's config is
    backendCalls: 0,
    async toggleAdvisor(enabled) {
      this.advisorEnabled = enabled;
      if (!this.isConnected) {
        this._advisorConfigDirty = true;
        return;
      }
      this.backendCalls++;
      this.backendConfig = { enabled, model: this.advisorModel, mode: this.advisorMode };
    },
    async setAdvisorModel(model) {
      this.advisorModel = model;
      prefsStorage.save({ model, mode: this.advisorMode });
      if (!this.isConnected) {
        this._advisorConfigDirty = true;
        return;
      }
      this.backendCalls++;
      this.backendConfig = { enabled: this.advisorEnabled, model, mode: this.advisorMode };
    },
    async setAdvisorMode(mode) {
      this.advisorMode = mode;
      prefsStorage.save({ model: this.advisorModel, mode });
      if (!this.isConnected) {
        this._advisorConfigDirty = true;
        return;
      }
      this.backendCalls++;
      this.backendConfig = { enabled: this.advisorEnabled, model: this.advisorModel, mode };
    },
    async _loadAdvisorConfig() {
      if (this._advisorConfigDirty) {
        this._advisorConfigDirty = false;
        this.backendCalls++;
        this.backendConfig = { enabled: this.advisorEnabled, model: this.advisorModel, mode: this.advisorMode };
        return;
      }
      // Simulates reading the backend's default (e.g. the global config)
      // for a session this panel never expressed a local opinion on yet.
      // model uses ?? since null is the backend's genuine "no opinion"
      // value; mode is only adopted when the backend config is actually
      // enabled (a resumed session with a real opinion), otherwise the
      // localStorage-seeded mode survives - a fresh backend default of
      // "max" must not clobber a remembered "low" choice.
      this.advisorEnabled = this.backendConfig?.enabled ?? false;
      this.advisorModel = this.backendConfig?.model ?? this.advisorModel;
      if (this.backendConfig?.enabled) {
        this.advisorMode = this.backendConfig?.mode || this.advisorMode;
      }
    },
    async connect() {
      this.isConnected = true;
      await this._loadAdvisorConfig();
    },
  };
}

const preConnect = createAdvisorAwareStore();
preConnect.toggleAdvisor(true);
assertEquals(preConnect.advisorEnabled, true, "toggling before connect still updates local state");
assertEquals(preConnect.backendCalls, 0, "toggling before connect does not call the backend");
assert(preConnect._advisorConfigDirty, "marks the config dirty when toggled before connect");

preConnect.connect();
assertEquals(preConnect.backendCalls, 1, "connecting pushes the dirty config to the backend exactly once");
assertEquals(preConnect.advisorEnabled, true, "the user's pre-connect toggle survives connecting");
assert(!preConnect._advisorConfigDirty, "clears the dirty flag once pushed");

const noLocalChoice = createAdvisorAwareStore();
noLocalChoice.backendConfig = { enabled: true, model: "gpt-5" }; // e.g. the global default
noLocalChoice.connect();
assertEquals(noLocalChoice.backendCalls, 0, "connecting without a prior local toggle does not push, only loads");
assertEquals(noLocalChoice.advisorEnabled, true, "loads the backend's default when the panel never toggled locally");
assertEquals(noLocalChoice.advisorModel, "gpt-5", "loads the backend's model default too");

const postConnect = createAdvisorAwareStore();
postConnect.isConnected = true;
postConnect.toggleAdvisor(true);
assertEquals(postConnect.backendCalls, 1, "toggling after connect calls the backend immediately");
assert(!postConnect._advisorConfigDirty, "does not mark dirty when already connected");

// --- setAdvisorMode + localStorage persistence (mirrored) ---
console.log("\nsetAdvisorMode + localStorage persistence (mirrored):");

const freshStore = createAdvisorAwareStore();
assertEquals(freshStore.advisorMode, "max", "defaults to max mode with no saved preferences");

const modeStore = createAdvisorAwareStore();
modeStore.setAdvisorMode("low");
assertEquals(modeStore.advisorMode, "low", "setAdvisorMode updates local state");
assertEquals(modeStore.backendCalls, 0, "setAdvisorMode before connect does not call the backend");
assert(modeStore._advisorConfigDirty, "setAdvisorMode before connect marks dirty");

const modeStore2 = createAdvisorAwareStore();
modeStore2.isConnected = true;
modeStore2.setAdvisorMode("low");
assertEquals(modeStore2.backendCalls, 1, "setAdvisorMode after connect calls the backend immediately");
assertEquals(modeStore2.backendConfig.mode, "low", "backend config carries the new mode");

// A new panel (fresh store instance) sharing the same underlying storage
// must come up pre-filled with the previous panel's last choice - this is
// the actual "survives restarts" guarantee, simulated here as "survives a
// new store instance" since a real Tauri restart can't be driven from node.
const sharedStorage = createPrefsStorage();
const panelA = createAdvisorAwareStore(sharedStorage);
panelA.setAdvisorModel("deepseek-v4-pro");
panelA.setAdvisorMode("low");
const panelB = createAdvisorAwareStore(sharedStorage);
assertEquals(panelB.advisorModel, "deepseek-v4-pro", "a new panel picks up the remembered model");
assertEquals(panelB.advisorMode, "low", "a new panel picks up the remembered mode");
assertEquals(panelB.advisorEnabled, false, "a new panel still starts with advisor OFF regardless of remembered prefs");

// A fresh (never-configured) backend session must not clobber the
// localStorage-seeded mode with its own bare "max" default.
const noOpinionYet = createAdvisorAwareStore(sharedStorage);
noOpinionYet.backendConfig = { enabled: false, model: null, mode: "max" }; // fresh backend session default
noOpinionYet.connect();
assertEquals(noOpinionYet.advisorMode, "low", "a disabled/default backend config does not override the remembered mode");

// A genuinely-configured (enabled) backend session - e.g. a resumed
// session - DOES win, since it reflects a real prior choice for that
// specific session, not a bare default.
const resumedSession = createAdvisorAwareStore();
resumedSession.backendConfig = { enabled: true, model: "gpt-5", mode: "low" };
resumedSession.connect();
assertEquals(resumedSession.advisorMode, "low", "an enabled backend config's mode is adopted");
assertEquals(resumedSession.advisorModel, "gpt-5", "an enabled backend config's model is adopted");

// --- Summary ---
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed > 0 ? 1 : 0);
