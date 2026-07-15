<template>
  <q-drawer
    :model-value="modelValue"
    @update:model-value="onUpdate"
    :width="isDesktop && !expanded ? 0 : 320"
    :breakpoint="1024"
    show-if-above
    side="right"
    bordered
    no-swipe-close
    class="mcp-drawer"
    :class="{ 'mcp-drawer--dark': $q.dark.isActive }"
  >
    <div class="mcp-drawer__inner">
      <!-- Header -->
      <div class="mcp-drawer__header">
        <div class="mcp-drawer__header-title">
          <q-icon name="dns" size="18px" class="q-mr-sm" color="primary" />
          {{ $t("mcp.title") }}
        </div>
        <div class="mcp-drawer__header-actions">
          <button
            type="button"
            class="mcp-drawer__header-btn"
            :disabled="isLoading"
            @click="onRefreshAll"
          >
            <q-icon name="refresh" size="16px" />
            <q-tooltip>{{ $t("mcp.refreshAll") }}</q-tooltip>
          </button>
          <button
            v-if="$q.screen.width < 1024"
            type="button"
            class="mcp-drawer__header-btn"
            @click="onUpdate(false)"
          >
            <q-icon name="close" size="18px" />
            <q-tooltip>{{ $t("common.close") }}</q-tooltip>
          </button>
        </div>
      </div>

      <!-- Edited files (current session) -->
      <div class="mcp-drawer__files">
        <div class="mcp-drawer__files-title">{{ $t("mcp.editedFilesTitle") }}</div>

        <div v-if="editedFiles.length === 0" class="mcp-drawer__files-empty-hint">
          {{ $t("mcp.editedFilesEmpty") }}
        </div>

        <template v-else>
          <div class="mcp-drawer__files-strip">
            <button
              v-for="file in editedFiles"
              :key="file.path"
              type="button"
              class="mcp-file-chip"
              :class="{ 'mcp-file-chip--active': expandedFilePath === file.path }"
              @click="toggleFile(file.path)"
            >
              <q-icon name="description" size="14px" />
              <span class="mcp-file-chip__name">{{ basename(file.path) }}</span>
              <span v-if="file.edits.length > 1" class="mcp-file-chip__badge">{{
                file.edits.length
              }}</span>
              <q-tooltip>{{ file.path }}</q-tooltip>
            </button>
          </div>

          <div v-if="expandedFile" class="mcp-drawer__diff-panel">
            <div class="mcp-drawer__diff-header">
              <span>{{ basename(expandedFile.path) }}</span>
              <q-tooltip>{{ expandedFile.path }}</q-tooltip>
              <q-btn
                round
                dense
                flat
                size="xs"
                icon="close"
                @click="expandedFilePath = null"
              />
            </div>
            <div v-for="(edit, i) in expandedFile.edits" :key="edit.id" class="mcp-drawer__diff-edit">
              <div v-if="expandedFile.edits.length > 1" class="mcp-drawer__diff-edit-label">
                {{ $t("mcp.editedFilesEditN", { n: i + 1 }) }}
              </div>
              <pre class="tool-diff-block"><span
                  v-for="(line, j) in editPreviewFor(edit)"
                  :key="j"
                  :class="'tool-diff-line--' + line.type"
                  >{{ line.text }}</span
                ></pre>
            </div>
          </div>
        </template>
      </div>

      <!-- Content -->
      <div class="mcp-drawer__body">
        <!-- Loading -->
        <div v-if="isLoading && backends.length === 0" class="mcp-drawer__center">
          <q-spinner-dots size="40px" color="primary" />
          <div class="mcp-drawer__hint">{{ $t("mcp.loading") }}</div>
        </div>

        <!-- Empty state -->
        <div v-else-if="!isLoading && backends.length === 0" class="mcp-drawer__center">
          <div class="mcp-drawer__empty-icon">
            <q-icon name="dns" size="40px" />
          </div>
          <div class="mcp-drawer__hint">{{ $t("mcp.empty") }}</div>
          <div class="mcp-drawer__hint-sub">{{ $t("mcp.emptyHint") }}</div>
        </div>

        <!-- Server list -->
        <div v-else class="mcp-drawer__list">
          <div
            v-for="backend in backends"
            :key="backend.name"
            class="mcp-card"
            :class="{
              'mcp-card--ok': backend.status === 'ok',
              'mcp-card--error': backend.status === 'error',
            }"
          >
            <div class="mcp-card__header">
              <div class="mcp-card__icon">
                <q-icon :name="backend.type === 'http' ? 'language' : 'terminal'" size="18px" />
              </div>
              <div class="mcp-card__meta">
                <div class="mcp-card__title">
                  {{ backend.name }}
                </div>
                <div class="mcp-card__subtitle">
                  <span
                    class="mcp-card__type"
                    :class="{
                      'mcp-card__type--http': backend.type === 'http',
                      'mcp-card__type--stdio': backend.type === 'stdio',
                    }"
                    >{{ backend.type }}</span
                  >
                  <span v-if="backend.url" class="mcp-card__url">{{
                    truncateUrl(backend.url)
                  }}</span>
                </div>
              </div>
              <div class="mcp-card__status">
                <q-icon
                  v-if="backend._checking"
                  name="hourglass_empty"
                  size="16px"
                  class="mcp-card__status-spin"
                />
                <q-icon
                  v-else-if="backend.status === 'ok'"
                  name="check_circle"
                  size="18px"
                  class="mcp-card__status-ok"
                />
                <q-icon
                  v-else-if="backend.status === 'error'"
                  name="error"
                  size="18px"
                  class="mcp-card__status-error"
                />
                <q-icon v-else name="cloud" size="18px" class="mcp-card__status-idle" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="mcp-drawer__footer">
        <div class="mcp-drawer__footer-hint">{{ $t("mcp.footerHint") }}</div>
      </div>
    </div>
  </q-drawer>

  <!-- Toggle button on the drawer's left edge. Rendered outside the q-drawer
       itself so it stays reachable even when the drawer is fully hidden
       (mobile overlay closed / desktop collapsed to width 0). -->
  <q-btn
    round
    dense
    unelevated
    size="sm"
    :icon="isOpenVisually ? 'chevron_right' : 'chevron_left'"
    class="mcp-drawer-toggle-btn"
    :style="{
      left: toggleLeft,
      top: isOpenVisually ? '3%' : '50%',
      transform: isOpenVisually ? 'translate(-50%, 0)' : 'translate(-50%, -50%)',
    }"
    @click="onToggleClick"
  >
    <q-tooltip>{{ isOpenVisually ? $t("mcp.collapsePanel") : $t("mcp.expandPanel") }}</q-tooltip>
  </q-btn>
</template>

<script setup>
import { ref, computed, watch } from "vue";
import { useQuasar } from "quasar";
import { useZeroStore } from "@/stores/zero-store";
import { storeToRefs } from "pinia";
import { getEditStrings } from "@/utils/edit-tools";

const $q = useQuasar();
const zeroStore = useZeroStore();
const {
  mcpBackends: backends,
  isLoadingMcp: isLoading,
  editedFiles,
} = storeToRefs(zeroStore);

const props = defineProps({
  modelValue: {
    type: Boolean,
    default: false,
  },
});

const expanded = ref(true);

const expandedFilePath = ref(null);
const expandedFile = computed(
  () => editedFiles.value.find((f) => f.path === expandedFilePath.value) || null,
);

function toggleFile(path) {
  expandedFilePath.value = expandedFilePath.value === path ? null : path;
}

function basename(path) {
  if (!path) return "";
  return path.split(/[\\/]/).pop();
}

// Kept independent from ToolCallMessage.vue's `editPreview` so this drawer
// never touches the hot chat-transcript path. Adapted to take the message
// explicitly since a file here can have several edits rendered as separate
// hunks.
function editPreviewFor(message) {
  const input = message.input || {};
  const editStrings = getEditStrings(input);
  if (editStrings) {
    return [
      ...editStrings.oldStr.split("\n").map((text) => ({ type: "removed", text: `- ${text}` })),
      ...editStrings.newStr.split("\n").map((text) => ({ type: "added", text: `+ ${text}` })),
    ];
  }
  if (typeof input.content === "string") {
    return input.content.split("\n").map((text) => ({ type: "added", text: `+ ${text}` }));
  }
  return [];
}

// Purely local interaction state (not derived data), so it needs its own
// reset on session switch - guards against a same-named file existing in
// both the old and new session (editedFiles itself already updates on its
// own via the store getter, no watcher needed for that part).
watch(
  () => zeroStore.currentSessionId,
  () => {
    expandedFilePath.value = null;
  },
);

const emit = defineEmits(["update:modelValue"]);

function onUpdate(value) {
  emit("update:modelValue", value);
}

// Below the breakpoint the drawer is an overlay controlled by modelValue
// (show/hide). At/above it, the drawer stays docked and "closing" just
// collapses its width to 0, so `expanded` drives the visual state instead.
const isDesktop = computed(() => $q.screen.width >= 1024);
const isOpenVisually = computed(() => (isDesktop.value ? expanded.value : props.modelValue));
const toggleLeft = computed(() => `${$q.screen.width - (isOpenVisually.value ? 320 : 0)}px`);

function onToggleClick() {
  if (isDesktop.value) {
    expanded.value = !expanded.value;
  } else {
    onUpdate(!props.modelValue);
  }
}

async function onRefreshAll() {
  await zeroStore.loadMcpBackends({ force: true });
  await zeroStore.checkAllMcpBackends();
}

function truncateUrl(url) {
  if (!url) return "";
  try {
    const u = new URL(url);
    return u.hostname;
  } catch {
    return url.length > 28 ? url.slice(0, 28) + "…" : url;
  }
}
</script>

<style scoped>
.mcp-drawer {
  background: var(--chat-card-bg, rgba(250, 250, 250, 0.94));
  transition: width 0.25s ease;
}

.mcp-drawer--dark {
  background: var(--chat-card-bg, rgba(30, 30, 30, 0.94));
}

.mcp-drawer__inner {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.mcp-drawer__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 16px 14px;
  border-bottom: 1px solid rgba(128, 128, 128, 0.14);
}

.mcp-drawer__header-title {
  display: flex;
  align-items: center;
  font-size: 0.95em;
  font-weight: 700;
  color: var(--chat-text);
  letter-spacing: 0.2px;
}

.mcp-drawer__header-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.mcp-drawer__header-btn {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: rgba(128, 128, 128, 0.85);
  cursor: pointer;
  transition:
    background 0.15s ease,
    color 0.15s ease,
    transform 0.1s ease;
}

.mcp-drawer__header-btn:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.14);
  color: var(--chat-text);
  transform: scale(1.06);
}

.mcp-drawer__header-btn:active:not(:disabled) {
  transform: scale(0.94);
}

.mcp-drawer__header-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.mcp-drawer__files {
  padding: 10px 14px 12px;
  border-bottom: 1px solid rgba(128, 128, 128, 0.14);
}

.mcp-drawer__files-title {
  font-size: 0.72em;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: var(--chat-text-muted);
  margin-bottom: 6px;
}

.mcp-drawer__files-empty-hint {
  font-size: 0.8em;
  color: var(--chat-text-muted);
}

.mcp-drawer__files-strip {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  padding-bottom: 2px;
}

.mcp-file-chip {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 140px;
  padding: 5px 9px;
  border-radius: 999px;
  border: 1px solid rgba(128, 128, 128, 0.18);
  background: rgba(128, 128, 128, 0.06);
  color: var(--chat-text);
  font-size: 0.78em;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.mcp-file-chip:hover {
  background: rgba(128, 128, 128, 0.12);
}

.mcp-file-chip--active {
  border-color: rgba(25, 210, 77, 0.4);
  background: rgba(25, 210, 77, 0.1);
}

.mcp-file-chip__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mcp-file-chip__badge {
  flex-shrink: 0;
  min-width: 15px;
  height: 15px;
  padding: 0 3px;
  border-radius: 999px;
  background: rgba(128, 128, 128, 0.2);
  color: var(--chat-text);
  font-size: 0.72em;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.mcp-drawer__diff-panel {
  margin-top: 10px;
  border: 1px solid rgba(128, 128, 128, 0.14);
  border-radius: 10px;
  background: rgba(128, 128, 128, 0.04);
  max-height: 260px;
  overflow-y: auto;
}

.mcp-drawer__diff-header {
  position: sticky;
  top: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  background: var(--chat-card-bg, rgba(250, 250, 250, 0.94));
  border-bottom: 1px solid rgba(128, 128, 128, 0.12);
  font-size: 0.78em;
  font-family: "JetBrains Mono", "Fira Code", Consolas, monospace;
  color: var(--chat-text-muted);
}

.mcp-drawer__diff-edit {
  padding: 4px 8px;
}

.mcp-drawer__diff-edit-label {
  font-size: 0.72em;
  color: var(--chat-text-muted);
  margin-top: 4px;
}

/* Duplicated verbatim from ToolCallMessage.vue's .tool-diff-block /
   .tool-diff-line--removed / .tool-diff-line--added - kept independent on
   purpose so the chat transcript's rendering path stays untouched. */
.tool-diff-block {
  margin: 0;
  font-size: 0.82em;
  line-height: 1.5;
  font-family: "JetBrains Mono", "Fira Code", Consolas, monospace;
  white-space: pre-wrap;
  word-break: break-word;
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

.mcp-drawer__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 12px 14px;
}

.mcp-drawer__list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.mcp-drawer__center {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 32px;
}

.mcp-drawer__empty-icon {
  width: 72px;
  height: 72px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(128, 128, 128, 0.08);
  border: 1px solid rgba(128, 128, 128, 0.12);
  color: rgba(128, 128, 128, 0.55);
  margin-bottom: 14px;
}

.mcp-drawer__hint {
  font-size: 0.9em;
  font-weight: 500;
  color: var(--chat-text);
  max-width: 220px;
  margin-top: 8px;
}

.mcp-drawer__hint-sub {
  font-size: 0.8em;
  color: var(--chat-text-muted);
  max-width: 220px;
  margin-top: 4px;
}

.mcp-card {
  border-radius: 14px;
  background: rgba(128, 128, 128, 0.06);
  border: 1px solid rgba(128, 128, 128, 0.12);
  transition:
    border-color 0.15s ease,
    background 0.15s ease,
    box-shadow 0.15s ease;
}

.mcp-card:hover {
  background: rgba(128, 128, 128, 0.1);
  border-color: rgba(128, 128, 128, 0.22);
}

.mcp-card--ok {
  border-color: rgba(33, 186, 69, 0.25);
}

.mcp-card--ok:hover {
  border-color: rgba(33, 186, 69, 0.38);
  box-shadow: 0 2px 10px rgba(33, 186, 69, 0.08);
}

.mcp-card--error {
  border-color: rgba(244, 67, 54, 0.22);
}

.mcp-card--error:hover {
  border-color: rgba(244, 67, 54, 0.35);
  box-shadow: 0 2px 10px rgba(244, 67, 54, 0.06);
}

.mcp-card__header {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  border: none;
  background: transparent;
  cursor: pointer;
  text-align: left;
  color: inherit;
  border-radius: 14px;
  transition: background 0.15s ease;
}

.mcp-card__icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(128, 128, 128, 0.12);
  color: rgba(128, 128, 128, 0.85);
  transition: background 0.15s ease;
}

.mcp-card--ok .mcp-card__icon {
  background: rgba(33, 186, 69, 0.12);
  color: #21ba45;
}

.mcp-card--error .mcp-card__icon {
  background: rgba(244, 67, 54, 0.12);
  color: #f44336;
}

.mcp-card__meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.mcp-card__title {
  font-size: 0.9em;
  font-weight: 600;
  color: var(--chat-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mcp-card__subtitle {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.78em;
  color: var(--chat-text-muted);
  min-width: 0;
}

.mcp-card__type {
  flex-shrink: 0;
  padding: 2px 7px;
  border-radius: 6px;
  font-size: 0.85em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  background: rgba(128, 128, 128, 0.12);
  color: rgba(128, 128, 128, 0.85);
}

.mcp-card__type--http {
  background: rgba(25, 118, 210, 0.1);
  color: #1976d2;
}

.mcp-card__type--stdio {
  background: rgba(242, 156, 55, 0.12);
  color: #f29c37;
}

.mcp-card__url {
  max-width: 110px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mcp-card__status {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  color: rgba(128, 128, 128, 0.65);
}

.mcp-card__status-ok {
  color: #21ba45;
}

.mcp-card__status-error {
  color: #f44336;
}

.mcp-card__status-idle {
  color: rgba(128, 128, 128, 0.55);
}

.mcp-card__status-spin {
  animation: mcp-spin 1.2s linear infinite;
  color: rgba(128, 128, 128, 0.65);
}

@keyframes mcp-spin {
  to {
    transform: rotate(360deg);
  }
}

.mcp-drawer__footer {
  padding: 10px 16px 14px;
  border-top: 1px solid rgba(128, 128, 128, 0.12);
}

.mcp-drawer__footer-hint {
  font-size: 0.75em;
  color: var(--chat-text-muted);
  text-align: center;
  line-height: 1.4;
}

/* Scrollbar discreto */
.mcp-drawer__body::-webkit-scrollbar {
  width: 6px;
}

.mcp-drawer__body::-webkit-scrollbar-track {
  background: transparent;
}

.mcp-drawer__body::-webkit-scrollbar-thumb {
  background: rgba(128, 128, 128, 0.25);
  border-radius: 3px;
}

.mcp-drawer__body::-webkit-scrollbar-thumb:hover {
  background: rgba(128, 128, 128, 0.4);
}

.mcp-drawer-toggle-btn {
  position: fixed;
  top: 50%;
  z-index: 3001;
  width: 26px;
  height: 26px;
  background: var(--chat-card-bg);
  border: 1px solid var(--chat-card-border);
  color: var(--chat-text-muted);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
  transition:
    top 0.15s ease,
    left 0.25s ease,
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease,
    transform 0.15s ease;
}

.mcp-drawer-toggle-btn:hover {
  background: rgba(25, 210, 77, 0.14);
  border-color: rgba(25, 210, 77, 0.4);
  color: #19d24d;
  transform: translate(-50%, -50%) scale(1.15) !important;
}
</style>
