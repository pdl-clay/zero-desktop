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
assert(enabledNoModel.includes("JÁ EXISTE"), "tells the executor the specialist already exists");
assert(enabledNoModel.includes("GenerateSpecialist"), "warns against (re)creating the specialist");
assert(enabledNoModel.includes("PROATIVAMENTE"), "tells the executor to consult on its own initiative");
assert(
  enabledNoModel.includes("Não espere o usuário pedir"),
  "makes explicit that no explicit user request is needed",
);
assert(enabledNoModel.includes("Seja eficiente"), "includes efficiency guidance");
assert(
  enabledNoModel.includes("Não consulte para tarefas triviais"),
  "warns against consulting for trivial tasks",
);

// The model lives in the specialist file's own frontmatter (synced before
// this prompt is built), not in the prompt text - the executor never needs
// it, and repeating it here risks going stale relative to the file.
const enabledWithModel = executorInstructionPrompt({ enabled: true, model: "claude-opus-4-1" });
assert(!enabledWithModel.includes("claude-opus-4-1"), "does not repeat the configured model in the prompt");

// --- executorInstructionPrompt: mode (max/low) ---
console.log("\nexecutorInstructionPrompt (mode):");

const defaultModePrompt = executorInstructionPrompt({ enabled: true, model: null });
assert(defaultModePrompt.includes("modo Max"), "missing/unrecognized mode defaults to max");

const maxModePrompt = executorInstructionPrompt({ enabled: true, model: null, mode: "max" });
assert(maxModePrompt.includes("Segurança e boas práticas"), "max mode lists the five broad categories");
assert(maxModePrompt.includes("PROATIVAMENTE"), "max mode keeps the proactive framing");

const lowModePrompt = executorInstructionPrompt({ enabled: true, model: null, mode: "low" });
assert(lowModePrompt.includes("modo Low"), "low mode is labeled");
assert(lowModePrompt.includes("Planejamento inicial de alto risco"), "low mode has the planning trigger");
assert(lowModePrompt.includes("Recuperação de falha repetida"), "low mode has the failure-recovery trigger");
assert(
  !lowModePrompt.includes("Segurança e boas práticas"),
  "low mode does not repeat max mode's broad categories",
);
assert(lowModePrompt.includes("\"name\": \"advisor\""), "low mode still explains the Task call shape");
assert(lowModePrompt.includes("JÁ EXISTE"), "low mode still warns the specialist already exists");
assert(maxModePrompt !== lowModePrompt, "max and low mode prompts differ");

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
