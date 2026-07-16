/**
 * Standalone tests for advisor-prompt.js utilities.
 * Run: node tests/advisor-prompt.test.js
 */

import {
  executorInstructionPrompt,
  isAdvisorConsultation,
  extractAdvisorPrompt,
} from "../src/utils/advisor-prompt.js";

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

console.log("advisor-prompt.js tests\n");

// --- executorInstructionPrompt ---
console.log("executorInstructionPrompt:");

assert(executorInstructionPrompt(null) === null, "returns null for null config");
assert(executorInstructionPrompt({}) === null, "returns null for undefined enabled");
assert(executorInstructionPrompt({ enabled: false }) === null, "returns null when disabled");

const enabledNoModel = executorInstructionPrompt({ enabled: true, model: null });
assert(typeof enabledNoModel === "string", "returns string when enabled");
assert(enabledNoModel.includes("<advisor_mode>"), "contains advisor_mode tag");
assert(enabledNoModel.includes("Task"), "mentions Task tool");
assert(enabledNoModel.includes("advisor"), "mentions advisor specialist");
assert(!enabledNoModel.includes("modelo recomendado"), "no model hint when model is null");

const enabledWithModel = executorInstructionPrompt({ enabled: true, model: "claude-opus-4-1" });
assert(enabledWithModel.includes("claude-opus-4-1"), "contains model name");
assert(enabledWithModel.includes("modelo recomendado"), "contains model hint label");

// --- isAdvisorConsultation ---
console.log("\nisAdvisorConsultation:");

assert(isAdvisorConsultation("Task", { name: "advisor" }) === true, "detects Task+advisor");
assert(isAdvisorConsultation("Task", { name: "explorer" }) === false, "rejects non-advisor name");
assert(isAdvisorConsultation("read_file", { name: "advisor" }) === false, "rejects non-Task tool");
assert(isAdvisorConsultation("Task", {}) === false, "rejects missing name");
assert(isAdvisorConsultation("Task", null) === false, "rejects null args");

// --- extractAdvisorPrompt ---
console.log("\nextractAdvisorPrompt:");

assert(extractAdvisorPrompt({ prompt: "test" }) === "test", "extracts prompt string");
assert(extractAdvisorPrompt({}) === null, "returns null when no prompt");
assert(extractAdvisorPrompt(null) === null, "returns null for null args");

// --- summary ---
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed > 0 ? 1 : 0);
