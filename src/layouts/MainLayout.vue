<template>
  <q-layout view="lHh LpR lFf">
    <q-drawer
      v-model="leftDrawerOpen"
      show-if-above
      :width="sessionPanelOpen ? 300 : 60"
      :breakpoint="700"
      bordered
      :class="$q.dark.isActive ? 'bg-dark' : 'bg-grey-2'"
    >
      <div class="row full-height">
        <!-- Left avatar column -->
        <div
          class="col-auto column items-center q-py-sm"
          style="width: 60px"
          :class="$q.dark.isActive ? 'bg-dark' : 'bg-grey-3'"
        >
          <div class="q-pa-none">
            <img src="/assets/zero-logo.png" alt="Zero" class="drawer-logo" />
          </div>

          <q-separator class="full-width q-mb-sm" />

          <div
            ref="workspaceColumnRef"
            class="workspace-column col"
            :class="{ 'is-dragging': isDragging }"
          >
            <div
              v-for="(ws, index) in workspacesStore.workspaces"
              :key="ws.path"
              class="workspace-avatar-wrapper q-mb-sm"
              :class="{
                'drag-source': isDragging && dragIndex === index,
                'drop-before': isDragging && dragInsertIndex === index,
                'drop-after':
                  isDragging &&
                  dragInsertIndex === workspacesStore.workspaces.length &&
                  index === workspacesStore.workspaces.length - 1,
              }"
              @mousedown="onAvatarMouseDown($event, index)"
            >
              <WorkspaceAvatar
                :name="ws.name"
                :color="workspaceColor(ws.name)"
                :is-active="ws.path === workspacesStore.activePath"
                :is-working="workspaceHasWorkingAgent(ws.path)"
                :status="workspaceBadge(ws.path)"
                @click="onSelectWorkspace(ws)"
              />
              <q-btn
                class="workspace-remove-btn"
                round
                dense
                flat
                size="xs"
                icon="close"
                color="negative"
                @click.stop="onRemoveWorkspace(ws)"
              >
                <q-tooltip>{{
                  $t("workspace.removeWorkspaceTooltip", { name: ws.name })
                }}</q-tooltip>
              </q-btn>
              <q-tooltip anchor="center right" self="center left" :offset="[8, 0]">
                <div class="text-weight-bold">{{ ws.name }}</div>
                <div class="text-caption">{{ ws.path }}</div>
              </q-tooltip>
            </div>

            <div class="workspace-avatar-wrapper">
              <q-btn
                round
                flat
                size="sm"
                icon="add"
                color="grey-7"
                class="workspace-avatar"
                style="width: 34px; height: 34px"
                @click="onBrowseAndAdd"
              >
                <q-tooltip>{{ $t("workspace.add") }}</q-tooltip>
              </q-btn>
            </div>
          </div>

          <q-separator class="full-width q-mt-sm" />

          <q-btn
            flat
            round
            dense
            :icon="$q.dark.isActive ? 'light_mode' : 'dark_mode'"
            color="grey-7"
            size="sm"
            class="q-mt-xs"
            @click="toggleTheme"
          >
            <q-tooltip>{{
              $q.dark.isActive ? $t("theme.lightMode") : $t("theme.darkMode")
            }}</q-tooltip>
          </q-btn>

          <q-btn
            flat
            round
            dense
            icon="settings"
            color="grey-7"
            size="sm"
            class="q-mt-xs q-mb-xs"
            @click="settingsDialogOpen = true"
          >
            <q-tooltip>{{ $t("settings.title") }}</q-tooltip>
          </q-btn>
        </div>
        <!-- Right session panel -->
        <div
          :class="['col column session-panel', { 'session-panel--closed': !sessionPanelOpen }]"
          style="min-width: 0"
        >
          <div class="row items-center q-px-sm q-py-xs panel-header">
            <span class="text-caption text-weight-medium panel-header__title">
              {{ workspacesStore.active?.name || "Zero" }}
            </span>
          </div>

          <div class="q-pa-sm col column" style="min-height: 0">
            <!-- Zero status -->
            <q-banner
              v-if="!zeroStore.hasZero"
              class="bg-negative text-white q-mb-xs rounded-borders"
              dense
            >
              {{ $t("chat.zeroNotFound") }}
            </q-banner>

            <template v-if="workspacesStore.hasActive">
              <q-splitter v-model="explorerSplitter" horizontal class="col" style="min-height: 0">
                <template #before>
                  <div class="column full-height" style="min-width: 0">
                    <div class="text-caption panel-section-label q-mb-sm" style="min-width: 0">
                      {{ $t("workspace.sessions", { count: currentSessions.length }) }}
                    </div>

                    <q-scroll-area class="col" style="min-width: 0">
                      <!-- Session list -->
                      <q-list dense class="q-gutter-y-xs">
                        <SessionListItem
                          v-for="session in currentSessions"
                          :key="session.session_id"
                          :session="session"
                        />

                        <div
                          v-if="currentSessions.length === 0"
                          class="text-center panel-empty q-pa-md"
                        >
                          <q-icon name="chat" size="28px" />
                          <div class="text-caption q-mt-xs">{{ $t("workspace.noSessions") }}</div>
                        </div>

                        <q-item
                          v-if="currentSessions.length > 0"
                          clickable
                          v-ripple
                          class="session-item session-item--new q-px-sm session-item-wrapper"
                          @click="onNewSession"
                        >
                          <q-item-section side>
                            <q-icon name="add_comment" size="16px" color="grey-5" />
                          </q-item-section>
                          <q-item-section>
                            <q-item-label class="text-body2">{{
                              $t("workspace.newSession")
                            }}</q-item-label>
                          </q-item-section>
                        </q-item>
                      </q-list>
                    </q-scroll-area>
                  </div>
                </template>

                <template #after>
                  <div class="column full-height" style="min-width: 0">
                    <div class="text-caption panel-section-label q-mb-sm" style="min-width: 0">
                      {{ $t("explorer.title") }}
                    </div>
                    <FileExplorerTree :root-path="workspacesStore.activePath" class="col" />
                  </div>
                </template>
              </q-splitter>
            </template>

            <template v-else>
              <div class="flex flex-center col text-grey-5 text-center">
                <div>
                  <q-icon name="folder_open" size="36px" />
                  <div class="text-caption q-mt-sm">{{ $t("workspace.noWorkspaces") }}</div>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>

      <!-- Toggle button overlaid on right edge -->
      <q-btn
        round
        dense
        unelevated
        size="sm"
        :icon="sessionPanelOpen ? 'chevron_left' : 'chevron_right'"
        class="session-toggle-btn"
        :style="{
          top: sessionPanelOpen ? '3%' : '50%',
          transform: sessionPanelOpen ? 'translateX(50%)' : 'translate(50%, -50%)',
        }"
        @click="sessionPanelOpen = !sessionPanelOpen"
      >
        <q-tooltip>{{
          sessionPanelOpen ? $t("workspace.closePanel") : $t("workspace.openPanel")
        }}</q-tooltip>
      </q-btn>
    </q-drawer>

    <!-- Main content -->
    <q-page-container>
      <q-page v-if="!workspacesStore.hasActive" class="flex flex-center">
        <div class="text-center text-grey-5">
          <img
            :src="$q.dark.isActive ? '/zero-completa.png' : '/zero-completa-white.png'"
            alt="Zero"
            style="width: auto; height: auto; margin-bottom: 8px"
          />
          <div class="text-h6 q-mt-md">{{ $t("workspace.select") }}</div>
        </div>
      </q-page>
      <SessionTileGrid v-else />
    </q-page-container>

    <McpDrawer
      v-if="workspacesStore.hasActive && sessionRuntime.focusedKeyFor(workspacesStore.activePath)"
      v-model="mcpDrawerOpen"
    />

    <SettingsDialog v-model="settingsDialogOpen" />

    <TerminalPanel v-if="workspacesStore.hasActive" />
  </q-layout>
</template>

<script setup>
import { ref, computed, onMounted, watch, provide } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";
import { useWorkspacesStore } from "@/stores/workspaces-store";
import { useSessionRuntimeStore, MAX_OPEN_PANELS } from "@/stores/session-runtime-store";
import { openOrFocusSession } from "@/stores/session-runtime-store";
import { open } from "@tauri-apps/plugin-dialog";
import {
  deleteSession as deleteSessionApi,
  renameSession as renameSessionApi,
} from "@/services/zero";
import SessionTileGrid from "@/components/SessionTileGrid.vue";
import McpDrawer from "@/components/McpDrawer.vue";
import SettingsDialog from "@/components/settings/SettingsDialog.vue";
import TerminalPanel from "@/components/terminal/TerminalPanel.vue";
import FileExplorerTree from "@/components/explorer/FileExplorerTree.vue";
import StatusBadge from "@/components/StatusBadge.vue";
import WorkspaceAvatar from "@/components/WorkspaceAvatar.vue";
import SessionIndicator from "@/components/SessionIndicator.vue";
import SessionListItem from "@/components/SessionListItem.vue";

const $q = useQuasar();
const { t } = useI18n();
const zeroStore = useZeroStore();
const workspacesStore = useWorkspacesStore();
const sessionRuntime = useSessionRuntimeStore();
const leftDrawerOpen = ref(true);
const mcpDrawerOpen = ref($q.screen.width >= 1024);
const settingsDialogOpen = ref(false);

const isSmallScreen = $q.screen.lt.md;
const sessionPanelOpen = ref(!isSmallScreen);
// Splits the left drawer's session column into sessions (top) and the
// file explorer tree (bottom) - percentage given to the sessions side.
const explorerSplitter = ref(60);

const dragIndex = ref(-1);
const dragInsertIndex = ref(-1);
const isDragging = ref(false);
const dragClone = ref(null);
const dragStartMouseX = ref(0);
const dragStartMouseY = ref(0);
const dragHoldTimer = ref(null);
const workspaceColumnRef = ref(null);

const DRAG_HOLD_DELAY = 200;

watch(
  () => $q.screen.width,
  (width) => {
    if (width < 1024) {
      sessionPanelOpen.value = false;
    } else {
      sessionPanelOpen.value = true;
    }
  },
);

const THEME_KEY = "zero-desktop-theme";

const workspaceColors = [
  "#5c6bc0",
  "#26a69a",
  "#ffa726",
  "#ec407a",
  "#42a5f5",
  "#7e57c2",
  "#66bb6a",
  "#ef5350",
  "#8d6e63",
  "#26c6da",
];

function workspaceColor(name) {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return workspaceColors[Math.abs(hash) % workspaceColors.length];
}

const SESSION_TITLE_MAX_CHARS = 20;

function truncateTitle(title) {
  if (!title) return "";
  return title.length > SESSION_TITLE_MAX_CHARS
    ? title.slice(0, SESSION_TITLE_MAX_CHARS) + "…"
    : title;
}

const { locale } = useI18n();

function formatDate(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  const currentLocale = locale.value;
  return (
    d.toLocaleDateString(currentLocale, { day: "2-digit", month: "2-digit", year: "2-digit" }) +
    " " +
    d.toLocaleTimeString(currentLocale, { hour: "2-digit", minute: "2-digit" })
  );
}

const currentSessions = computed(
  () => workspacesStore.sessionsByPath[workspacesStore.activePath] ?? [],
);

function sessionKeyFor(sessionId) {
  const meta = sessionRuntime.keyMeta;
  for (const [key, info] of Object.entries(meta)) {
    if (info.sessionId === sessionId) return key;
  }
  return null;
}

function sessionMeta(session) {
  const key = sessionKeyFor(session.session_id);
  if (!key) return null;
  return sessionRuntime.keyMeta[key];
}

function sessionWorkingStatus(session) {
  return sessionMeta(session)?.workingStatus ?? null;
}

function sessionAttention(session) {
  return !!sessionMeta(session)?.hasPendingPermission;
}

function sessionBadge(session) {
  if (sessionAttention(session)) return "attention";
  if (sessionWorkingStatus(session)) return "working";
  return null;
}

function workspaceHasWorkingAgent(path) {
  return Object.values(sessionRuntime.keyMeta).some(
    (m) => m.cwd === path && m.workingStatus && !m.hasPendingPermission,
  );
}

function isSessionOpen(session) {
  const key = sessionKeyFor(session.session_id);
  return key ? sessionRuntime.isOpen(key) : false;
}

function workspaceBadge(path) {
  const meta = sessionRuntime.keyMeta;
  let hasAttention = false;
  for (const info of Object.values(meta)) {
    if (info.cwd !== path) continue;
    if (info.hasPendingPermission) hasAttention = true;
  }
  return hasAttention ? "attention" : null;
}

onMounted(async () => {
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === "dark") {
    $q.dark.set(true);
  }
  await zeroStore.locateZero();
  zeroStore.loadMcpBackends();
  if (workspacesStore.activePath) {
    await workspacesStore.loadSessions(workspacesStore.activePath);
  }
  checkForUpdatesInBackground();
});

// Silent startup update check: never blocks first paint, never restarts the
// app without explicit user confirmation - only surfaces a dismissible
// notification once a new version has already been downloaded and installed.
// See docs/en/architecture/decisions/005-tauri-updater-for-appimage-self-update.md.
async function checkForUpdatesInBackground() {
  await zeroStore.loadAppInfo();
  if (!zeroStore.isAppImageRuntime) return;
  try {
    await zeroStore.checkForUpdates({ silent: true });
    if (!zeroStore.updateAvailable) return;
    await zeroStore.downloadAndInstallUpdate();
    $q.notify({
      type: "positive",
      message: t("settings.updateReadyMessage", { version: zeroStore.updateInfo.version }),
      timeout: 0,
      actions: [
        {
          label: t("settings.restartNow"),
          color: "white",
          handler: () => zeroStore.restartToApplyUpdate(),
        },
      ],
    });
  } catch {
    // Silent by design - the user can retry manually from Settings.
  }
}

function toggleTheme() {
  $q.dark.toggle();
  localStorage.setItem(THEME_KEY, $q.dark.isActive ? "dark" : "light");
}

watch(
  () => workspacesStore.activePath,
  async (newPath) => {
    if (newPath) {
      await workspacesStore.loadSessions(newPath);
    }
  },
);

async function onSelectWorkspace(ws) {
  sessionPanelOpen.value = true;
  workspacesStore.select(ws.path);

  const hasOpenSessionForWorkspace = Object.values(sessionRuntime.keyMeta).some(
    (m) => m.cwd === ws.path,
  );
  if (!hasOpenSessionForWorkspace) {
    const key = crypto.randomUUID();
    const result = await openOrFocusSession(key, ws.path, null);
    if (result?.error === "SESSION_CAP_REACHED") {
      $q.notify({
        type: "warning",
        message: t("workspace.sessionCapReached", { max: MAX_OPEN_PANELS }),
        position: "top",
      });
    }
  }
}

async function onSelectSession(session) {
  const cwd = workspacesStore.activePath;
  if (!cwd) return;

  const result = await openOrFocusSession(session.session_id, cwd, session.session_id);
  if (result?.error === "SESSION_CAP_REACHED") {
    $q.notify({
      type: "warning",
      message: t("workspace.sessionCapReached", { max: MAX_OPEN_PANELS }),
      position: "top",
    });
    return;
  }

  if ($q.screen.width < 1024) {
    sessionPanelOpen.value = false;
  }
}

async function onDeleteSession(session) {
  const key = sessionKeyFor(session.session_id);
  if (key && sessionRuntime.isOpen(key)) {
    await sessionRuntime.stopAndDispose(key);
  }
  await deleteSessionApi(session.session_id);
  if (workspacesStore.activePath) {
    await workspacesStore.loadSessions(workspacesStore.activePath);
  }
}

function onRenameSession(session) {
  $q.dialog({
    title: t("workspace.renameSession"),
    prompt: {
      model: session.title || "",
      type: "text",
    },
    cancel: true,
    persistent: false,
  }).onOk(async (title) => {
    await renameSessionApi(session.session_id, title);
    if (workspacesStore.activePath) {
      await workspacesStore.loadSessions(workspacesStore.activePath);
    }
  });
}

// Consumed by SessionListItem.vue (recursive - one instance per row at any
// depth) via inject("sessionListActions"), instead of prop-drilling these
// eight functions down through every level of subagent/advisor nesting.
provide("sessionListActions", {
  isSessionOpen,
  sessionWorkingStatus,
  sessionAttention,
  truncateTitle,
  formatDate,
  onSelectSession,
  onRenameSession,
  onDeleteSession,
});

async function onNewSession() {
  const cwd = workspacesStore.activePath;
  if (!cwd) return;

  const key = crypto.randomUUID();
  const result = await openOrFocusSession(key, cwd, null);
  if (result?.error === "SESSION_CAP_REACHED") {
    $q.notify({
      type: "warning",
      message: t("workspace.sessionCapReached", { max: MAX_OPEN_PANELS }),
      position: "top",
    });
    return;
  }

  const existingKeys = Object.entries(sessionRuntime.keyMeta)
    .filter(([_, m]) => m.cwd === cwd && m.workingStatus)
    .map(([k]) => k);
  if (existingKeys.length > 0) {
    $q.notify({
      type: "info",
      message: t("workspace.sameWorkspaceWarning"),
      position: "top",
      timeout: 5000,
    });
  }

  if ($q.screen.width < 1024) {
    sessionPanelOpen.value = false;
  }
}

async function onBrowseAndAdd() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: t("workspace.addWorkspaceTitle"),
  });
  if (selected) {
    workspacesStore.add(selected);
  }
}

async function onRemoveWorkspace(ws) {
  workspacesStore.remove(ws.path);
}

function onAvatarMouseDown(event, index) {
  if (event.button !== 0) return;

  dragIndex.value = index;
  dragInsertIndex.value = index;
  isDragging.value = false;
  dragStartMouseX.value = event.clientX;
  dragStartMouseY.value = event.clientY;

  const wrapper = event.currentTarget;
  wrapper.addEventListener("click", preventClickAfterDrag, true);

  dragHoldTimer.value = setTimeout(() => {
    if (dragIndex.value === -1) return;
    isDragging.value = true;
    createDragClone();
  }, DRAG_HOLD_DELAY);

  document.addEventListener("mousemove", onDocumentMouseMove);
  document.addEventListener("mouseup", onDocumentMouseUp);
}

function preventClickAfterDrag(event) {
  if (isDragging.value) {
    event.stopPropagation();
    event.preventDefault();
  }
  event.currentTarget.removeEventListener("click", preventClickAfterDrag, true);
}

function cancelDrag() {
  if (dragHoldTimer.value) {
    clearTimeout(dragHoldTimer.value);
    dragHoldTimer.value = null;
  }
  if (dragClone.value) {
    dragClone.value.remove();
    dragClone.value = null;
  }
  dragIndex.value = -1;
  dragInsertIndex.value = -1;
  isDragging.value = false;
  document.removeEventListener("mousemove", onDocumentMouseMove);
  document.removeEventListener("mouseup", onDocumentMouseUp);
}

function onDocumentMouseMove(event) {
  if (dragIndex.value === -1) return;

  const deltaX = Math.abs(event.clientX - dragStartMouseX.value);
  const deltaY = Math.abs(event.clientY - dragStartMouseY.value);

  // Antes de ativar o drag, se o usuário se mover muito consideramos um clique/swipe e cancelamos
  if (!isDragging.value && dragHoldTimer.value && (deltaX > 15 || deltaY > 15)) {
    cancelDrag();
    return;
  }

  if (!isDragging.value) return;

  updateDragClonePosition(event.clientX, event.clientY);
  updateDragInsertIndex(event.clientY);
}

function createDragClone() {
  const column = workspaceColumnRef.value;
  if (!column) return;
  const wrappers = column.querySelectorAll(".workspace-avatar-wrapper");
  const sourceWrapper = wrappers[dragIndex.value];
  if (!sourceWrapper) return;

  const sourceAvatar = sourceWrapper.querySelector(".workspace-avatar");
  if (!sourceAvatar) return;

  const rect = sourceAvatar.getBoundingClientRect();
  const clone = sourceAvatar.cloneNode(true);
  clone.classList.add("dragging-clone");
  clone.style.position = "fixed";
  clone.style.left = `${rect.left}px`;
  clone.style.top = `${rect.top}px`;
  clone.style.width = `${rect.width}px`;
  clone.style.height = `${rect.height}px`;
  clone.style.margin = "0";
  clone.style.zIndex = "9999";
  clone.style.pointerEvents = "none";
  clone.style.opacity = "0.9";

  document.body.appendChild(clone);
  dragClone.value = clone;
}

function updateDragClonePosition(clientX, clientY) {
  if (!dragClone.value) return;
  const rect = dragClone.value.getBoundingClientRect();
  dragClone.value.style.left = `${clientX - rect.width / 2}px`;
  dragClone.value.style.top = `${clientY - rect.height / 2}px`;
}

function updateDragInsertIndex(clientY) {
  const column = workspaceColumnRef.value;
  if (!column) return;
  const wrappers = Array.from(column.querySelectorAll(".workspace-avatar-wrapper"));
  let newIndex = workspacesStore.workspaces.length;
  for (let i = 0; i < wrappers.length; i++) {
    const rect = wrappers[i].getBoundingClientRect();
    const midpoint = rect.top + rect.height / 2;
    if (clientY < midpoint) {
      newIndex = i;
      break;
    }
  }
  dragInsertIndex.value = newIndex;
}

function onDocumentMouseUp() {
  if (dragHoldTimer.value) {
    clearTimeout(dragHoldTimer.value);
    dragHoldTimer.value = null;
  }

  if (isDragging.value && dragIndex.value !== -1) {
    const from = dragIndex.value;
    const to = dragInsertIndex.value;
    if (to !== from && to !== from + 1) {
      workspacesStore.reorder(from, to);
    }
  }

  if (dragClone.value) {
    dragClone.value.remove();
    dragClone.value = null;
  }

  dragIndex.value = -1;
  dragInsertIndex.value = -1;

  document.removeEventListener("mousemove", onDocumentMouseMove);
  document.removeEventListener("mouseup", onDocumentMouseUp);

  // Mantém isDragging true até depois do evento de click que virá em seguida
  setTimeout(() => {
    isDragging.value = false;
  }, 0);
}
</script>

<style scoped>
.drawer-logo {
  width: 32px;
  height: 32px;
  object-fit: contain;
}

.workspace-column {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  overflow-y: auto;
  padding: 8px;
  min-height: 0;
}

.workspace-column.is-dragging {
  cursor: grabbing;
}

.workspace-column.is-dragging .workspace-avatar-wrapper {
  cursor: grabbing;
}

.dragging-clone {
  pointer-events: none;
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35);
  transition: none !important;
}

.dragging-clone .workspace-avatar__ring {
  display: none;
}

.workspace-avatar-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: grab;
  transition:
    transform 0.15s ease,
    opacity 0.15s ease;
}

.workspace-avatar-wrapper:active {
  cursor: grabbing;
}

.workspace-avatar-wrapper.drag-source {
  opacity: 0.35;
  transform: scale(0.85);
}

.workspace-avatar-wrapper.drop-before {
  position: relative;
}

.workspace-avatar-wrapper.drop-before::before {
  content: "";
  position: absolute;
  top: -8px;
  left: 50%;
  transform: translateX(-50%);
  width: 34px;
  height: 4px;
  background: #19d24d;
  border-radius: 2px;
  box-shadow: 0 0 6px rgba(25, 210, 77, 0.7);
  z-index: 2;
}

.workspace-avatar-wrapper.drop-after {
  position: relative;
}

.workspace-avatar-wrapper.drop-after::after {
  content: "";
  position: absolute;
  bottom: -8px;
  left: 50%;
  transform: translateX(-50%);
  width: 34px;
  height: 4px;
  background: #19d24d;
  border-radius: 2px;
  box-shadow: 0 0 6px rgba(25, 210, 77, 0.7);
  z-index: 2;
}

.workspace-remove-btn {
  position: absolute;
  bottom: 27px;
  right: -8px;
  z-index: 1;
  opacity: 0;
  transform: scale(0.4);
  transition: all 0.15s ease;
}

.workspace-avatar-wrapper:hover .workspace-remove-btn {
  opacity: 1;
  transform: scale(1);
}

.session-panel {
  max-width: 240px;
  opacity: 1;
  transition:
    max-width 0.25s ease,
    opacity 0.2s ease,
    padding 0.25s ease;
  overflow: hidden;
}

.session-panel--closed {
  max-width: 0;
  opacity: 0;
  padding: 0 !important;
}

.panel-header {
  border-bottom: 1px solid var(--chat-card-border);
}

.panel-header__title {
  color: var(--chat-text);
}

.panel-section-label {
  color: var(--chat-text-muted);
}

.panel-empty {
  color: var(--chat-text-muted);
}

/* Base look for the always-inline "new session" row - SessionListItem.vue
   defines the same .session-item/.session-item-wrapper rules in its own
   scoped style for the recursive rows, but Vue's scoped CSS doesn't reach
   this row since it isn't rendered by that component. */
.session-item-wrapper {
  position: relative;
  min-width: 0;
  width: 100%;
}

.session-item {
  border-radius: 10px;
  border: 1px solid transparent;
  color: var(--chat-text);
  width: 100%;
  min-width: 0;
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.session-item:hover {
  background: var(--chat-card-bg);
  border-color: var(--chat-card-border);
}

.session-item--new {
  color: var(--chat-text-muted);
}

.session-toggle-btn {
  position: absolute;
  right: 0;
  z-index: 100;
  width: 26px;
  height: 26px;
  background: var(--chat-card-bg);
  border: 1px solid var(--chat-card-border);
  color: var(--chat-text-muted);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
  transition:
    top 0.15s ease,
    transform 0.15s ease,
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}

.session-toggle-btn:hover {
  background: rgba(25, 210, 77, 0.14);
  border-color: rgba(25, 210, 77, 0.4);
  color: #19d24d;
  transform: translateX(50%) translatey(-10%) scale(1.15) !important;
}
</style>
