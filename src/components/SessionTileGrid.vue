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
      <div :class="paneCardClass">
        <SessionPaneHeader :session-key="openKeys[0]" />
        <ChatView :session-key="openKeys[0]" :key="openKeys[0]" class="col" />
      </div>
    </template>

    <template v-else-if="openKeys.length === 2">
      <q-splitter v-model="splitterH" class="fit">
        <template #before>
          <div :class="paneCardClass">
            <SessionPaneHeader :session-key="openKeys[0]" />
            <ChatView :session-key="openKeys[0]" :key="openKeys[0]" class="col" />
          </div>
        </template>
        <template #after>
          <div :class="paneCardClass">
            <SessionPaneHeader :session-key="openKeys[1]" />
            <ChatView :session-key="openKeys[1]" :key="openKeys[1]" class="col" />
          </div>
        </template>
      </q-splitter>
    </template>

    <template v-else-if="openKeys.length === 3">
      <q-splitter v-model="splitterOuter" class="fit">
        <template #before>
          <div :class="paneCardClass">
            <SessionPaneHeader :session-key="openKeys[0]" />
            <ChatView :session-key="openKeys[0]" :key="openKeys[0]" class="col" />
          </div>
        </template>
        <template #after>
          <q-splitter v-model="splitterInnerV" horizontal class="fit">
            <template #before>
              <div :class="paneCardClass">
                <SessionPaneHeader :session-key="openKeys[1]" />
                <ChatView :session-key="openKeys[1]" :key="openKeys[1]" class="col" />
              </div>
            </template>
            <template #after>
              <div :class="paneCardClass">
                <SessionPaneHeader :session-key="openKeys[2]" />
                <ChatView :session-key="openKeys[2]" :key="openKeys[2]" class="col" />
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
              <div :class="paneCardClass">
                <SessionPaneHeader :session-key="openKeys[0]" />
                <ChatView :session-key="openKeys[0]" :key="openKeys[0]" class="col" />
              </div>
            </template>
            <template #after>
              <div :class="paneCardClass">
                <SessionPaneHeader :session-key="openKeys[1]" />
                <ChatView :session-key="openKeys[1]" :key="openKeys[1]" class="col" />
              </div>
            </template>
          </q-splitter>
        </template>
        <template #after>
          <q-splitter v-model="splitterRightV" horizontal class="fit">
            <template #before>
              <div :class="paneCardClass">
                <SessionPaneHeader :session-key="openKeys[2]" />
                <ChatView :session-key="openKeys[2]" :key="openKeys[2]" class="col" />
              </div>
            </template>
            <template #after>
              <div :class="paneCardClass">
                <SessionPaneHeader :session-key="openKeys[3]" />
                <ChatView :session-key="openKeys[3]" :key="openKeys[3]" class="col" />
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
import { useWorkspacesStore } from "@/stores/workspaces-store";
import ChatView from "@/components/ChatView.vue";
import SessionPaneHeader from "@/components/chat/SessionPaneHeader.vue";

const $q = useQuasar();
const runtime = useSessionRuntimeStore();
const workspacesStore = useWorkspacesStore();

// Each workspace only ever sees its OWN panels in this grid - a panel
// belonging to a different workspace keeps running/tracked in the
// background (see session-runtime-store.js's openKeys, which stays a
// single flat list across all workspaces), it just isn't rendered here
// until its workspace becomes active again. That's also what makes the
// layout "formation" (which keys, in which order) come back exactly as it
// was when switching back - nothing is torn down on the way out.
const openKeys = computed(() => runtime.visibleKeys(workspacesStore.activePath));

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
const gridHeight = computed(() => `${$q.screen.height}px`);

// Each pane gets the same floating-card treatment as ChatInput (rounded
// corners, translucent tinted background, hairline border) instead of being
// a flush, borderless rectangle - matches the visual language already
// established for the input bar rather than introducing a second style.
const paneCardClass = computed(() => [
  "session-pane-card column",
  { "session-pane-card--dark": $q.dark.isActive },
]);

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

:deep(.q-splitter__panel) {
  padding: 6px;
}
</style>
