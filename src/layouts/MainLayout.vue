<template>
  <q-layout view="lHh Lpr lFf">
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
              <div
                class="workspace-avatar cursor-pointer"
                :class="{ active: ws.path === workspacesStore.activePath }"
                :style="avatarStyle(ws)"
                @click="onSelectWorkspace(ws)"
              >
                {{ ws.name.charAt(0).toUpperCase() }}
              </div>
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

          <q-btn flat round dense icon="settings" color="grey-7" size="sm" class="q-mt-xs q-mb-xs">
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
              <div class="text-caption panel-section-label q-mb-sm" style="min-width: 0">
                {{ $t("workspace.sessions", { count: zeroStore.sessions.length }) }}
              </div>

              <q-scroll-area class="col" style="min-width: 0">
                <!-- Session list -->
                <q-list dense class="q-gutter-y-xs">
                  <div
                    v-for="session in zeroStore.sessions"
                    :key="session.session_id"
                    class="session-item-wrapper"
                  >
                    <q-item
                      clickable
                      v-ripple
                      :class="[
                        'session-item q-px-sm',
                        {
                          'session-item--active': session.session_id === zeroStore.currentSessionId,
                        },
                      ]"
                      @click="onSelectSession(session)"
                    >
                      <q-item-section side>
                        <q-icon :name="sessionIcon(session.kind)" size="16px" color="grey-6" />
                      </q-item-section>

                      <q-item-section class="session-item__content">
                        <q-item-label class="text-body2 session-item__title" lines="1">
                          {{ truncateTitle(session.title) || session.session_id.slice(-8) }}
                          <q-tooltip v-if="session.title" anchor="top middle" self="bottom middle">
                            {{ session.title }}
                          </q-tooltip>
                        </q-item-label>
                        <q-item-label
                          caption
                          class="row items-center q-gutter-x-xs session-item__meta"
                        >
                          <span v-if="session.model_id" class="ellipsis">{{
                            session.model_id
                          }}</span>
                          <span v-if="session.model_id">&middot;</span>
                          <span class="ellipsis">{{ formatDate(session.created_at) }}</span>
                        </q-item-label>
                      </q-item-section>
                    </q-item>

                    <q-btn
                      class="session-rename-btn"
                      round
                      dense
                      flat
                      size="xs"
                      icon="edit"
                      color="grey-7"
                      @click.stop="onRenameSession(session)"
                    >
                      <q-tooltip>{{ $t("workspace.renameSession") }}</q-tooltip>
                    </q-btn>
                    <q-btn
                      class="session-remove-btn"
                      round
                      dense
                      flat
                      size="xs"
                      icon="close"
                      color="negative"
                      @click.stop="onDeleteSession(session)"
                    >
                      <q-tooltip>{{ $t("workspace.deleteSession") }}</q-tooltip>
                    </q-btn>
                  </div>

                  <div
                    v-if="zeroStore.sessions.length === 0"
                    class="text-center panel-empty q-pa-md"
                  >
                    <q-icon name="chat" size="28px" />
                    <div class="text-caption q-mt-xs">{{ $t("workspace.noSessions") }}</div>
                  </div>

                  <q-item
                    v-if="zeroStore.sessions.length > 0"
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
      <ChatView v-else :workspace-path="workspacesStore.activePath" />
    </q-page-container>
  </q-layout>
</template>

<script setup>
import { ref, onMounted, watch } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";
import { useWorkspacesStore } from "@/stores/workspaces-store";
import { open } from "@tauri-apps/plugin-dialog";
import ChatView from "@/components/ChatView.vue";

const $q = useQuasar();
const { t } = useI18n();
const zeroStore = useZeroStore();
const workspacesStore = useWorkspacesStore();
const leftDrawerOpen = ref(true);

const isSmallScreen = $q.screen.lt.md;
const sessionPanelOpen = ref(!isSmallScreen);

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

function avatarStyle(ws) {
  const isActive = ws.path === workspacesStore.activePath;
  const color = workspaceColor(ws.name);
  return {
    backgroundColor: color,
    width: isActive ? "40px" : "34px",
    height: isActive ? "40px" : "34px",
    fontSize: isActive ? "16px" : "12px",
    boxShadow: isActive ? `0 0 0 3px #fff, 0 0 0 5px ${color}` : "none",
  };
}

const SESSION_TITLE_MAX_CHARS = 20;

function truncateTitle(title) {
  if (!title) return "";
  return title.length > SESSION_TITLE_MAX_CHARS
    ? title.slice(0, SESSION_TITLE_MAX_CHARS) + "…"
    : title;
}

function sessionIcon(kind) {
  switch (kind) {
    case "fork":
      return "call_split";
    case "child":
      return "subdirectory_arrow_right";
    default:
      return "chat_bubble_outline";
  }
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

onMounted(async () => {
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === "dark") {
    $q.dark.set(true);
  }
  console.log(
    "[MainLayout] workspaces loaded:",
    workspacesStore.workspaces.length,
    workspacesStore.workspaces,
  );
  await zeroStore.locateZero();
});

function toggleTheme() {
  $q.dark.toggle();
  localStorage.setItem(THEME_KEY, $q.dark.isActive ? "dark" : "light");
}

watch(
  () => workspacesStore.activePath,
  async (newPath, oldPath) => {
    if (oldPath && zeroStore.isConnected) {
      await zeroStore.stopSession();
    }
    if (newPath) {
      zeroStore.currentWorkspace = newPath;
      zeroStore.currentSessionId = null;
      zeroStore.messages = [];
      zeroStore.currentResponse = "";
      zeroStore.currentThinking = "";
      zeroStore.currentPlan = [];
      zeroStore.runInProgress = false;
      await zeroStore.loadSessions(newPath);
    }
  },
);

async function onSelectWorkspace(ws) {
  sessionPanelOpen.value = true;
  workspacesStore.select(ws.path);
}

async function onSelectSession(session) {
  const cwd = workspacesStore.activePath;
  if (!cwd) return;

  await zeroStore.startSession(cwd, session.session_id);
  await zeroStore.openSession(session.session_id);
  await zeroStore.loadSessions(cwd);

  if ($q.screen.width < 1024) {
    sessionPanelOpen.value = false;
  }
}

async function onDeleteSession(session) {
  console.log("[MainLayout] onDeleteSession:", session.session_id, session);
  await zeroStore.removeSession(session.session_id);
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
  }).onOk((title) => {
    zeroStore.renameSession(session.session_id, title);
  });
}

async function onNewSession() {
  const cwd = workspacesStore.activePath;
  if (!cwd) return;
  await zeroStore.startSession(cwd);
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

.workspace-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  color: #fff;
  font-weight: 700;
  line-height: 1;
  transition: all 0.2s ease;
  user-select: none;
  flex-shrink: 0;
}

.workspace-avatar:hover {
  opacity: 0.85;
  transform: scale(1.12);
}

.workspace-avatar.active {
  opacity: 1;
  transform: scale(1);
}

.session-item-wrapper {
  position: relative;
  min-width: 0;
  width: 100%;
}

.session-remove-btn,
.session-rename-btn {
  position: absolute;
  top: 2px;
  z-index: 1;
  opacity: 0;
  transform: scale(0.4);
  transition: all 0.15s ease;
}

.session-remove-btn {
  right: 2px;
}

.session-rename-btn {
  right: 24px;
}

.session-item-wrapper:hover .session-remove-btn,
.session-item-wrapper:hover .session-rename-btn {
  opacity: 1;
  transform: scale(1);
}

.session-item-wrapper :deep(.q-item__section--main) {
  min-width: 0;
  overflow: hidden;
}

.session-item__content {
  min-width: 0 !important;
  overflow: hidden;
}

.session-item__title,
.session-item__meta {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-item__meta span {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.session-item--active {
  background: rgba(25, 210, 77, 0.12);
  border-color: rgba(25, 210, 77, 0.4);
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
