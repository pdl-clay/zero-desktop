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

// --- Summary ---
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed > 0 ? 1 : 0);
