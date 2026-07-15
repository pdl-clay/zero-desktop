<template>
  <div :class="['tool-call-card q-mb-sm', cardClass]">
    <div class="row items-center q-px-sm q-py-xs">
      <q-spinner-dots
        v-if="message.status === 'running'"
        size="14px"
        color="info"
        class="q-mr-sm"
      />
      <q-icon
        v-else
        :name="isError ? 'error' : 'check_circle'"
        size="14px"
        :color="isError ? 'negative' : 'positive'"
        class="q-mr-xs"
      />
      <q-icon :name="toolIcon" size="14px" color="info" class="q-mr-xs" />
      <span class="text-caption text-weight-medium tool-name">{{ message.toolName }}</span>
      <span class="text-caption text-grey-6 q-ml-xs">{{ statusLabel }}</span>
      <q-tooltip
        v-if="message.status === 'running' && !editPreview && !planItems && inputSummary"
        max-width="400px"
        anchor="bottom left"
        self="top left"
      >
        <pre class="tool-input-preview">{{ inputSummary }}</pre>
      </q-tooltip>
      <q-space />
      <q-btn
        v-if="isDone && message.result && !editPreview && !planItems"
        round
        dense
        flat
        size="xs"
        :icon="showResult ? 'expand_less' : 'expand_more'"
        color="grey-5"
        @click="showResult = !showResult"
      >
        <q-tooltip>{{ showResult ? $t("chat.showLess") : $t("chat.showMore") }}</q-tooltip>
      </q-btn>
      <q-btn
        v-if="isDone && message.result"
        round
        dense
        flat
        size="xs"
        icon="content_copy"
        color="grey-5"
        @click="onCopy"
      >
        <q-tooltip>{{ $t("chat.copy") }}</q-tooltip>
      </q-btn>
    </div>

    <!-- edit_file / write_file: show the actual change, not just "success" -->
    <div v-if="editPreview" class="tool-diff q-px-sm q-pb-sm">
      <div v-if="editPreview.path" class="tool-diff-path text-caption">{{ editPreview.path }}</div>
      <pre class="tool-diff-block"><span
          v-for="(line, i) in editPreview.lines"
          :key="i"
          :class="'tool-diff-line--' + line.type"
          >{{ line.text }}</span
        ></pre>
      <div v-if="isError && message.result" class="tool-diff-error text-caption q-mt-xs">
        {{ cleanHookOutput(message.result) }}
      </div>
    </div>

    <!-- update_plan: render as a checklist instead of a JSON/text dump -->
    <div v-else-if="planItems" class="tool-plan q-px-sm q-pb-sm">
      <div v-for="(item, i) in planItems" :key="i" class="tool-plan-item row items-start q-mb-xs">
        <q-icon
          :name="planIcon(item.status)"
          :color="planColor(item.status)"
          size="16px"
          class="q-mr-xs q-mt-xs"
        />
        <div>
          <div :class="['text-body2', item.status === 'completed' ? 'tool-plan-done' : '']">
            {{ item.content }}
          </div>
          <div v-if="item.notes" class="text-caption tool-plan-notes">{{ item.notes }}</div>
        </div>
      </div>
    </div>

    <!-- everything else: generic collapsible text result -->
    <div v-else-if="showResult && message.result" class="tool-result-body q-px-sm q-pb-sm">
      <pre :class="['tool-result-content', isError ? 'tool-result-content--error' : '']">{{
        truncatedResult
      }}</pre>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from "vue";
import { useQuasar } from "quasar";
import { copyToClipboard } from "quasar";
import { useI18n } from "vue-i18n";
import { planIcon, planColor } from "@/utils/plan";
import { isEditTool, getEditStrings } from "@/utils/edit-tools";

const props = defineProps({
  message: { type: Object, required: true },
});

const $q = useQuasar();
const { t: $t } = useI18n();
const showResult = ref(false);

const MAX_RESULT_LINES = 25;

const isError = computed(() => props.message.status === "error");
const isDone = computed(() => props.message.status === "completed" || isError.value);

const cardClass = computed(() => ({
  "tool-call-card--dark": $q.dark.isActive,
  "tool-call-card--running": props.message.status === "running",
  "tool-call-card--completed": props.message.status === "completed",
  "tool-call-card--error": isError.value,
}));

const statusLabel = computed(() => {
  if (props.message.status === "running") return $t("chat.toolRunning");
  return isError.value ? $t("chat.toolFailed") : $t("chat.toolCompleted");
});

const toolIcon = computed(() => {
  const name = props.message.toolName || "";
  if (name.startsWith("mcp_")) return "extension";
  const icons = {
    read_file: "description",
    write_file: "edit",
    edit_file: "edit",
    bash: "terminal",
    exec_command: "terminal",
    write_stdin: "keyboard",
    glob: "search",
    grep: "find_in_page",
    list_directory: "folder",
    web_fetch: "language",
    web_search: "travel_explore",
    tool_search: "manage_search",
    update_plan: "checklist",
    task: "account_tree",
    question: "help_outline",
  };
  return icons[name] || "build";
});

const inputSummary = computed(() => {
  const input = props.message.input;
  if (!input || typeof input !== "object") return "";
  const entries = Object.entries(input).filter(
    ([, v]) => v !== undefined && v !== null && v !== "",
  );
  if (entries.length === 0) return "";
  return entries
    .map(([k, v]) => `${k}: ${typeof v === "object" ? JSON.stringify(v) : v}`)
    .join("\n");
});

// zero's edit_file (verified against the real CLI: {path, edit_type:"text",
// old_str, new_str}) only reports "Successfully edited X" in the result -
// the actual change is only present in the tool_call's own input. Rebuild a
// diff-like view from it instead of just showing the success sentence.
const editPreview = computed(() => {
  const name = props.message.toolName;
  const input = props.message.input || {};

  if (isEditTool(name)) {
    const editStrings = getEditStrings(input);
    if (editStrings) {
      const lines = [
        ...editStrings.oldStr.split("\n").map((text) => ({ type: "removed", text: `- ${text}` })),
        ...editStrings.newStr.split("\n").map((text) => ({ type: "added", text: `+ ${text}` })),
      ];
      return { path: input.path || "", lines };
    }

    // write_file's exact arg name isn't confirmed against the real CLI yet
    // (didn't get a live sample) - `content` is the common convention, and
    // this degrades harmlessly to the generic result view if it's wrong.
    if (typeof input.content === "string") {
      const lines = input.content.split("\n").map((text) => ({ type: "added", text: `+ ${text}` }));
      return { path: input.path || "", lines };
    }
  }

  return null;
});

const planItems = computed(() => {
  if (props.message.toolName !== "update_plan") return null;
  const plan = props.message.input?.plan;
  return Array.isArray(plan) && plan.length > 0 ? plan : null;
});

function cleanHookOutput(value) {
  if (typeof value !== "string") return value;
  // zero appends a "Hook output:\n{}" trailer to nearly every tool result;
  // it's noise (always empty in practice) so we strip it before display.
  const idx = value.indexOf("\n\nHook output:");
  return idx >= 0 ? value.slice(0, idx).trimEnd() : value;
}

const formattedResult = computed(() => {
  const result = props.message.result || "";
  if (typeof result !== "string") return JSON.stringify(result, null, 2);

  const cleaned = cleanHookOutput(result);
  const trimmed = cleaned.trim();
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      return JSON.stringify(JSON.parse(trimmed), null, 2);
    } catch {
      // Not valid JSON; fall through to the plain cleaned string.
    }
  }
  return cleaned;
});

const truncatedResult = computed(() => {
  const lines = formattedResult.value.split("\n");
  if (lines.length > MAX_RESULT_LINES) {
    return lines.slice(0, MAX_RESULT_LINES).join("\n") + "\n...";
  }
  return formattedResult.value;
});

function onCopy() {
  copyToClipboard(cleanHookOutput(props.message.result) || "");
}
</script>

<style scoped>
.tool-call-card {
  border-radius: 6px;
  border: 1px solid var(--chat-card-border);
  background: var(--chat-card-bg);
  transition:
    border-color 0.3s ease,
    background 0.3s ease;
}
.tool-name {
  color: var(--chat-text);
}
.tool-call-card--running {
  border-color: rgba(33, 150, 243, 0.25);
  background: rgba(33, 150, 243, 0.04);
}
.tool-call-card--dark.tool-call-card--running {
  border-color: rgba(33, 150, 243, 0.18);
  background: rgba(33, 150, 243, 0.03);
}
.tool-call-card--error {
  border-color: rgba(244, 67, 54, 0.25);
  background: rgba(244, 67, 54, 0.03);
}
.tool-input-preview {
  margin: 0;
  font-size: 0.78em;
  white-space: pre-wrap;
  word-break: break-all;
  color: inherit;
}
.tool-result-body {
  border-top: 1px solid var(--chat-card-border);
}
.tool-result-content {
  margin: 0;
  font-size: 0.82em;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 400px;
  overflow-y: auto;
  background: var(--chat-card-bg);
  color: var(--chat-text);
  padding: 8px;
  border-radius: 6px;
}
.tool-result-content--error {
  background: rgba(244, 67, 54, 0.08);
  color: #d32f2f;
}
body.body--dark .tool-result-content--error {
  background: rgba(244, 67, 54, 0.1);
  color: #ff8a80;
}

.tool-diff {
  border-top: 1px solid var(--chat-card-border);
}
.tool-diff-path {
  color: var(--chat-text-muted);
  margin: 6px 0 2px;
  font-family: "JetBrains Mono", "Fira Code", Consolas, monospace;
}
.tool-diff-block {
  margin: 0;
  font-size: 0.82em;
  line-height: 1.5;
  font-family: "JetBrains Mono", "Fira Code", Consolas, monospace;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 320px;
  overflow-y: auto;
  border-radius: 6px;
  overflow-x: hidden;
}
.tool-diff-line--removed {
  display: block;
  background: rgba(244, 67, 54, 0.1);
  color: #d32f2f;
  padding: 0 6px;
}
body.body--dark .tool-diff-line--removed {
  background: rgba(244, 67, 54, 0.12);
  color: #ff8a80;
}
.tool-diff-line--added {
  display: block;
  background: rgba(25, 210, 77, 0.12);
  color: #1a9c3f;
  padding: 0 6px;
}
body.body--dark .tool-diff-line--added {
  background: rgba(25, 210, 77, 0.14);
  color: #6fe396;
}
.tool-diff-error {
  color: var(--chat-text-muted);
}

.tool-plan {
  border-top: 1px solid var(--chat-card-border);
  padding-top: 6px;
}
.tool-plan-item {
  color: var(--chat-text);
}
.tool-plan-done {
  color: var(--chat-text-muted);
  text-decoration: line-through;
}
.tool-plan-notes {
  color: var(--chat-text-muted);
}
</style>
