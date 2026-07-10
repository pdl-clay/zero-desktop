<template>
  <div :class="['tool-call-card q-mb-sm', cardClass]">
    <!-- Running state -->
    <div v-if="message.status === 'running'" class="row items-center q-px-sm q-py-xs">
      <q-spinner-dots size="14px" color="info" class="q-mr-sm" />
      <q-icon :name="toolIcon" size="14px" color="info" class="q-mr-xs" />
      <span class="text-caption text-weight-medium">{{ message.toolName }}</span>
      <span class="text-caption text-grey-6 q-ml-xs">{{ $t("chat.toolRunning") }}</span>
      <q-tooltip v-if="inputSummary" max-width="400px" anchor="bottom left" self="top left">
        <pre class="tool-input-preview">{{ inputSummary }}</pre>
      </q-tooltip>
    </div>

    <!-- Completed / error state -->
    <div v-else-if="isDone" class="tool-call-completed">
      <div class="row items-center q-px-sm q-py-xs">
        <q-icon
          :name="isError ? 'error' : 'check_circle'"
          size="14px"
          :color="isError ? 'negative' : 'positive'"
          class="q-mr-xs"
        />
        <q-icon :name="toolIcon" size="14px" color="info" class="q-mr-xs" />
        <span class="text-caption text-weight-medium tool-name">{{ message.toolName }}</span>
        <span class="text-caption text-grey-6 q-ml-xs">{{
          isError ? $t("chat.toolFailed") : $t("chat.toolCompleted")
        }}</span>
        <q-space />
        <q-btn
          v-if="message.result"
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
          v-if="message.result"
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
      <div v-if="showResult && message.result" class="tool-result-body q-px-sm q-pb-sm">
        <pre :class="['tool-result-content', isError ? 'tool-result-content--error' : '']">{{
          truncatedResult
        }}</pre>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from "vue";
import { useQuasar } from "quasar";
import { copyToClipboard } from "quasar";

const props = defineProps({
  message: { type: Object, required: true },
});

const $q = useQuasar();
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

const truncatedResult = computed(() => {
  const result = props.message.result || "";
  if (typeof result !== "string") return JSON.stringify(result, null, 2);
  const lines = result.split("\n");
  if (lines.length > MAX_RESULT_LINES) {
    return lines.slice(0, MAX_RESULT_LINES).join("\n") + "\n...";
  }
  return result;
});

function onCopy() {
  copyToClipboard(props.message.result || "");
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
.tool-call-completed {
  border-radius: 6px;
}
.tool-input-preview {
  margin: 0;
  font-size: 0.78em;
  white-space: pre-wrap;
  word-break: break-all;
  color: inherit;
}
.tool-result-body {
  border-top: 1px solid rgba(0, 0, 0, 0.06);
}
.tool-call-card--dark .tool-result-body {
  border-color: rgba(255, 255, 255, 0.06);
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
</style>
