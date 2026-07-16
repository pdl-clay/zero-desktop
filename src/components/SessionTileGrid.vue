<template>
  <div class="session-tile-grid" :style="{ height: gridHeight }">
    <template v-if="openKeys.length === 0">
      <div class="flex flex-center fit text-grey-5 text-center">
        <div>
          <q-icon name="chat_bubble_outline" size="48px" />
          <div class="text-body1 q-mt-sm">{{ $t("chat.emptySession") }}</div>
          <div class="text-caption">{{ $t("chat.emptySessionSubtitle") }}</div>
        </div>
      </div>
    </template>

    <template v-else-if="openKeys.length === 1">
      <div :class="paneClass(openKeys[0])" v-bind="paneDropHandlers(openKeys[0])">
        <SessionPaneHeader :session-key="openKeys[0]" />
        <ChatView
          :session-key="openKeys[0]"
          :key="openKeys[0]"
          class="col"
          @focus-input="onFocusInput(openKeys[0])"
        />
      </div>
    </template>

    <template v-else-if="openKeys.length === 2">
      <q-splitter v-model="splitterH" class="fit">
        <template #before>
          <div :class="paneClass(openKeys[0])" v-bind="paneDropHandlers(openKeys[0])">
            <SessionPaneHeader :session-key="openKeys[0]" />
            <ChatView
              :session-key="openKeys[0]"
              :key="openKeys[0]"
              class="col"
              @focus-input="onFocusInput(openKeys[0])"
            />
          </div>
        </template>
        <template #after>
          <div :class="paneClass(openKeys[1])" v-bind="paneDropHandlers(openKeys[1])">
            <SessionPaneHeader :session-key="openKeys[1]" />
            <ChatView
              :session-key="openKeys[1]"
              :key="openKeys[1]"
              class="col"
              @focus-input="onFocusInput(openKeys[1])"
            />
          </div>
        </template>
      </q-splitter>
    </template>

    <template v-else-if="openKeys.length === 3">
      <q-splitter v-model="splitterOuter" class="fit">
        <template #before>
          <div :class="paneClass(openKeys[0])" v-bind="paneDropHandlers(openKeys[0])">
            <SessionPaneHeader :session-key="openKeys[0]" />
            <ChatView
              :session-key="openKeys[0]"
              :key="openKeys[0]"
              class="col"
              @focus-input="onFocusInput(openKeys[0])"
            />
          </div>
        </template>
        <template #after>
          <q-splitter v-model="splitterInnerV" horizontal class="fit">
            <template #before>
              <div :class="paneClass(openKeys[1])" v-bind="paneDropHandlers(openKeys[1])">
                <SessionPaneHeader :session-key="openKeys[1]" />
                <ChatView
                  :session-key="openKeys[1]"
                  :key="openKeys[1]"
                  class="col"
                  @focus-input="onFocusInput(openKeys[1])"
                />
              </div>
            </template>
            <template #after>
              <div :class="paneClass(openKeys[2])" v-bind="paneDropHandlers(openKeys[2])">
                <SessionPaneHeader :session-key="openKeys[2]" />
                <ChatView
                  :session-key="openKeys[2]"
                  :key="openKeys[2]"
                  class="col"
                  @focus-input="onFocusInput(openKeys[2])"
                />
              </div>
            </template>
          </q-splitter>
        </template>
      </q-splitter>
    </template>

    <template v-else-if="openKeys.length >= 4">
      <q-splitter v-model="splitterOuter" class="fit">
        <template #before>
          <q-splitter v-model="splitterLeftV" horizontal class="fit">
            <template #before>
              <div :class="paneClass(openKeys[0])" v-bind="paneDropHandlers(openKeys[0])">
                <SessionPaneHeader :session-key="openKeys[0]" />
                <ChatView
                  :session-key="openKeys[0]"
                  :key="openKeys[0]"
                  class="col"
                  @focus-input="onFocusInput(openKeys[0])"
                />
              </div>
            </template>
            <template #after>
              <div :class="paneClass(openKeys[1])" v-bind="paneDropHandlers(openKeys[1])">
                <SessionPaneHeader :session-key="openKeys[1]" />
                <ChatView
                  :session-key="openKeys[1]"
                  :key="openKeys[1]"
                  class="col"
                  @focus-input="onFocusInput(openKeys[1])"
                />
              </div>
            </template>
          </q-splitter>
        </template>
        <template #after>
          <q-splitter v-model="splitterRightV" horizontal class="fit">
            <template #before>
              <div :class="paneClass(openKeys[2])" v-bind="paneDropHandlers(openKeys[2])">
                <SessionPaneHeader :session-key="openKeys[2]" />
                <ChatView
                  :session-key="openKeys[2]"
                  :key="openKeys[2]"
                  class="col"
                  @focus-input="onFocusInput(openKeys[2])"
                />
              </div>
            </template>
            <template #after>
              <div :class="paneClass(openKeys[3])" v-bind="paneDropHandlers(openKeys[3])">
                <SessionPaneHeader :session-key="openKeys[3]" />
                <ChatView
                  :session-key="openKeys[3]"
                  :key="openKeys[3]"
                  class="col"
                  @focus-input="onFocusInput(openKeys[3])"
                />
              </div>
            </template>
          </q-splitter>
        </template>
      </q-splitter>
    </template>
  </div>
</template>

<script setup>
import { ref, computed } from "vue";
import { useQuasar } from "quasar";
import { useSessionRuntimeStore } from "@/stores/session-runtime-store";
import { useTerminalRuntimeStore } from "@/stores/terminal-runtime-store";
import { useWorkspacesStore } from "@/stores/workspaces-store";
import { attachFileToPanel } from "@/utils/attach-file-to-panel";
import ChatView from "@/components/ChatView.vue";
import SessionPaneHeader from "@/components/chat/SessionPaneHeader.vue";

const $q = useQuasar();
const runtime = useSessionRuntimeStore();
const terminalRuntime = useTerminalRuntimeStore();
const workspacesStore = useWorkspacesStore();

// Each workspace only ever sees its OWN panels in this grid - a panel
// belonging to a different workspace keeps running/tracked in the
// background (see session-runtime-store.js's openKeys, which stays a
// single flat list across all workspaces), it just isn't rendered here
// until its workspace becomes active again. That's also what makes the
// layout "formation" (which keys, in which order) come back exactly as it
// was when switching back - nothing is torn down on the way out.
const openKeys = computed(() => runtime.visibleKeys(workspacesStore.activePath));

// Marks a pane as "the one the user is working in" - both on an explicit
// textarea focus (ChatView's own @focus-input emit) and on a plain
// mousedown anywhere in the pane (scrolling the transcript, clicking a tool
// call, etc.), since "focused" here means "the terminal panel's cite action
// should target this one", not literally "caret is in the textarea".
function onFocusInput(key) {
  if (!key) return;
  runtime.focusPanel(key, workspacesStore.activePath);
}

// The other half of the file explorer's hybrid citation flow (see
// FileExplorerTree.vue): dropping a file directly onto a pane targets that
// exact panel, regardless of which one is currently focused - the dragged
// node's path travels via a custom dataTransfer MIME type set on dragstart.
const dragOverKey = ref(null);

function onDragOver(key) {
  dragOverKey.value = key;
}

function onDragLeave(key) {
  if (dragOverKey.value === key) dragOverKey.value = null;
}

async function onDropFile(event, key) {
  dragOverKey.value = null;
  const path = event.dataTransfer?.getData("application/x-zero-file");
  if (!path || !key) return;
  try {
    await attachFileToPanel(path, key);
  } catch (error) {
    $q.notify({ type: "negative", message: String(error), position: "top" });
  }
}

function paneDropHandlers(key) {
  return {
    onMousedownCapture: () => onFocusInput(key),
    onDragover: (event) => {
      event.preventDefault();
      onDragOver(key);
    },
    onDragleave: () => onDragLeave(key),
    onDrop: (event) => onDropFile(event, key),
  };
}

// q-page-container never gets an explicit CSS height of its own (Quasar only
// gives it padding for drawer offsets), so the `.fit` (height:100%) chain
// down through the nested q-splitters has nothing definite to resolve
// against - every percentage-based height/flex-basis inside silently
// collapses (verified empirically: dragging or programmatically overriding
// a splitter pane's height had zero effect, always snapping back). Anchoring
// one explicit pixel height here, exactly like QPage's own styleFn does for
// a single page, makes every percentage below it resolve correctly.
//
// This has to be a real CSS class (below) rather than Quasar's `.fit`
// utility, because `.fit` sets `height: 100% !important` - which silently
// wins over this inline style and was the reason an earlier version of this
// fix (still using `.fit` alongside the inline height) had no effect at all.
//
// Subtracts the terminal panel's reserved height when it's open, the same
// way this height already accounts for Quasar's own screen height - the
// terminal panel is a custom fixed-position element (Quasar drawers can't
// dock to the bottom), so nothing else shrinks this grid for it.
const gridHeight = computed(() => {
  const reserved = terminalRuntime.panelOpen ? terminalRuntime.panelHeightPx : 0;
  return `${$q.screen.height - reserved}px`;
});

// Each pane gets the same floating-card treatment as ChatInput (rounded
// corners, translucent tinted background, hairline border) instead of being
// a flush, borderless rectangle - matches the visual language already
// established for the input bar rather than introducing a second style.
function paneClass(key) {
  return [
    "session-pane-card column",
    {
      "session-pane-card--dark": $q.dark.isActive,
      "session-pane-card--drop-target": dragOverKey.value === key,
    },
  ];
}

const splitterH = ref(50);
const splitterOuter = ref(50);
const splitterInnerV = ref(50);
const splitterLeftV = ref(50);
const splitterRightV = ref(50);
</script>

<style scoped>
.session-tile-grid {
  position: relative;
  overflow: hidden;
  width: 100%;
  padding: 6px;
  box-sizing: border-box;
}

.session-pane-card {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  border-radius: 16px;
  border: 1px solid rgba(128, 128, 128, 0.16);
  background: rgba(128, 128, 128, 0.09);
  overflow: hidden;
  transition:
    border-color 0.15s ease,
    background 0.15s ease;
}

.session-pane-card--dark {
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.1);
}

.session-pane-card--drop-target {
  border-color: rgba(25, 210, 77, 0.5);
  background: rgba(25, 210, 77, 0.08);
}

:deep(.q-splitter__panel) {
  padding: 6px;
}
</style>
