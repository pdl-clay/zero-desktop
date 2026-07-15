<template>
  <div>
    <!-- Rendered even while collapsed (v-show, not v-if) so every open tab's
         TerminalHost - and thus its live xterm.js instance / scrollback -
         stays mounted regardless of the panel's open/closed state. -->
    <div
      v-show="terminalRuntime.panelOpen"
      class="terminal-panel"
      :class="{ 'terminal-panel--dark': $q.dark.isActive }"
      :style="{
        height: terminalRuntime.panelHeightPx + 'px',
        left: leftInset + 'px',
        right: rightInset + 'px',
      }"
    >
      <div class="terminal-panel__handle" @mousedown="onHandleMouseDown"></div>

      <div class="terminal-panel__toolbar">
        <TerminalTabStrip
          :tabs="visibleTabs"
          :active-key="activeKey"
          :key-meta="terminalRuntime.keyMeta"
          @focus="onFocusTab"
          @close="onCloseTab"
          @new-tab="onNewTab"
        />
        <button type="button" class="terminal-panel__cite-btn" @click="onCite">
          <q-icon name="content_paste_go" size="16px" />
          <q-tooltip>{{ $t("terminal.citeToChat") }}</q-tooltip>
        </button>
      </div>

      <div class="terminal-panel__body">
        <TerminalHost
          v-for="key in terminalRuntime.openKeys"
          :key="key"
          v-show="key === activeKey"
          :terminal-key="key"
          :cwd="terminalRuntime.keyMeta[key]?.cwd"
        />
        <div v-if="visibleTabs.length === 0" class="terminal-panel__empty">
          <q-icon name="terminal" size="32px" />
          <button type="button" class="terminal-panel__empty-btn" @click="onNewTab">
            {{ $t("terminal.newTab") }}
          </button>
        </div>
      </div>
    </div>

    <q-btn
      round
      dense
      unelevated
      size="sm"
      :icon="terminalRuntime.panelOpen ? 'keyboard_arrow_down' : 'keyboard_arrow_up'"
      class="terminal-panel-toggle-btn"
      :style="{
        bottom: (terminalRuntime.panelOpen ? terminalRuntime.panelHeightPx : 0) + 'px',
        left: toggleLeftPx + 'px',
      }"
      @click="terminalRuntime.panelOpen = !terminalRuntime.panelOpen"
    >
      <q-tooltip>{{
        terminalRuntime.panelOpen ? $t("terminal.collapsePanel") : $t("terminal.expandPanel")
      }}</q-tooltip>
    </q-btn>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { useWorkspacesStore } from "@/stores/workspaces-store";
import { useSessionRuntimeStore } from "@/stores/session-runtime-store";
import { useTerminalRuntimeStore } from "@/stores/terminal-runtime-store";
import { useTerminalSessionStore } from "@/stores/terminal-session-store";
import { useZeroSessionStore } from "@/stores/zero-session-store";
import TerminalTabStrip from "@/components/terminal/TerminalTabStrip.vue";
import TerminalHost from "@/components/terminal/TerminalHost.vue";

const $q = useQuasar();
const { t } = useI18n();
const workspacesStore = useWorkspacesStore();
const sessionRuntime = useSessionRuntimeStore();
const terminalRuntime = useTerminalRuntimeStore();

const visibleTabs = computed(() => terminalRuntime.visibleKeys(workspacesStore.activePath));
const activeKey = computed(() => terminalRuntime.focusedKeyFor(workspacesStore.activePath));

// Quasar's own layout system already computes exactly how much space the
// left sidebar and the right McpDrawer reserve (as padding-left/padding-right
// on .q-page-container - the same thing that already keeps SessionTileGrid
// correctly bounded between them), including all of their breakpoint/
// overlay-vs-push logic. Reading it back here, rather than re-deriving both
// drawers' widths from their own local state, is what keeps this panel's
// bounds correct without duplicating (and inevitably drifting from) that
// logic - a plain `left: 0; right: 0` fixed panel used to render full-width
// and get covered by both drawers instead of sitting between them.
const leftInset = ref(0);
const rightInset = ref(0);
let pageContainerObserver = null;

function updateInsets() {
  const el = document.querySelector(".q-page-container");
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const style = getComputedStyle(el);
  leftInset.value = rect.left + parseFloat(style.paddingLeft || "0");
  rightInset.value = window.innerWidth - rect.right + parseFloat(style.paddingRight || "0");
}

onMounted(() => {
  updateInsets();
  const el = document.querySelector(".q-page-container");
  // A ResizeObserver's default (content-box) fires on a padding-only change
  // too, even when the element's own outer width doesn't move - which is
  // exactly what happens here when a drawer toggles - so this alone covers
  // both drawer state changes and plain window resizes.
  if (el && typeof ResizeObserver !== "undefined") {
    pageContainerObserver = new ResizeObserver(updateInsets);
    pageContainerObserver.observe(el);
  }
});

onBeforeUnmount(() => {
  pageContainerObserver?.disconnect();
  pageContainerObserver = null;
});

const toggleLeftPx = computed(() => {
  const contentWidth = $q.screen.width - leftInset.value - rightInset.value;
  return leftInset.value + contentWidth / 2;
});

function onNewTab() {
  const key = crypto.randomUUID();
  terminalRuntime.openTab(key, workspacesStore.activePath);
}

function onFocusTab(key) {
  terminalRuntime.focusTab(key, workspacesStore.activePath);
}

function onCloseTab(key) {
  terminalRuntime.closeTab(key);
}

// Targets whichever chat panel currently has focus (same resolution
// McpDrawer.vue uses for its edited-files list) - inserts the focused
// terminal tab's output as a fenced code block into that panel's draft
// text, not the active terminal into itself or a hardcoded panel.
function onCite() {
  const workspacePath = workspacesStore.activePath;
  const termKey = activeKey.value;
  const chatKey = sessionRuntime.focusedKeyFor(workspacePath);
  if (!termKey || !chatKey) {
    $q.notify({ type: "warning", message: t("terminal.citeNoTarget"), position: "top" });
    return;
  }
  const text = useTerminalSessionStore(termKey).extractCiteText();
  if (!text) return;
  const chatStore = useZeroSessionStore(chatKey);
  const block = "```\n" + text + "\n```";
  chatStore.draftText = chatStore.draftText ? `${chatStore.draftText}\n\n${block}` : block;
}

let dragStartY = 0;
let dragStartHeight = 0;
let isDraggingHandle = false;

function onHandleMouseDown(event) {
  isDraggingHandle = true;
  dragStartY = event.clientY;
  dragStartHeight = terminalRuntime.panelHeightPx;
  document.addEventListener("mousemove", onHandleMouseMove);
  document.addEventListener("mouseup", onHandleMouseUp);
}

function onHandleMouseMove(event) {
  if (!isDraggingHandle) return;
  // Dragging the handle up (smaller clientY) should grow the panel.
  const delta = dragStartY - event.clientY;
  terminalRuntime.setPanelHeight(dragStartHeight + delta);
}

function onHandleMouseUp() {
  isDraggingHandle = false;
  document.removeEventListener("mousemove", onHandleMouseMove);
  document.removeEventListener("mouseup", onHandleMouseUp);
}
</script>

<style scoped>
.terminal-panel {
  position: fixed;
  bottom: 0;
  z-index: 900;
  display: flex;
  flex-direction: column;
  background: var(--chat-card-bg, rgba(250, 250, 250, 0.96));
  border-top: 1px solid var(--chat-card-border, rgba(128, 128, 128, 0.18));
  box-shadow: 0 -2px 10px rgba(0, 0, 0, 0.08);
}

.terminal-panel--dark {
  background: var(--chat-card-bg, rgba(24, 24, 24, 0.97));
}

.terminal-panel__handle {
  flex-shrink: 0;
  height: 6px;
  cursor: ns-resize;
}

.terminal-panel__handle:hover {
  background: rgba(25, 210, 77, 0.2);
}

.terminal-panel__toolbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  border-bottom: 1px solid rgba(128, 128, 128, 0.14);
}

.terminal-panel__cite-btn {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  margin-right: 8px;
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
    color 0.15s ease;
}

.terminal-panel__cite-btn:hover {
  background: rgba(25, 210, 77, 0.14);
  color: #19d24d;
}

.terminal-panel__body {
  flex: 1;
  min-height: 0;
  position: relative;
}

.terminal-panel__body > .terminal-host {
  position: absolute;
  inset: 0;
}

.terminal-panel__empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--chat-text-muted);
}

.terminal-panel__empty-btn {
  border: 1px solid rgba(128, 128, 128, 0.25);
  border-radius: 999px;
  padding: 6px 16px;
  background: transparent;
  color: var(--chat-text);
  cursor: pointer;
  font-size: 0.85em;
  transition: background 0.15s ease;
}

.terminal-panel__empty-btn:hover {
  background: rgba(25, 210, 77, 0.1);
}

.terminal-panel-toggle-btn {
  position: fixed;
  z-index: 901;
  width: 26px;
  height: 26px;
  transform: translate(-50%, 50%);
  background: var(--chat-card-bg);
  border: 1px solid var(--chat-card-border);
  color: var(--chat-text-muted);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
  transition:
    bottom 0.15s ease,
    left 0.15s ease,
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease,
    transform 0.15s ease;
}

.terminal-panel-toggle-btn:hover {
  background: rgba(25, 210, 77, 0.14);
  border-color: rgba(25, 210, 77, 0.4);
  color: #19d24d;
  transform: translate(-50%, 50%) scale(1.15) !important;
}
</style>
