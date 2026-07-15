<template>
  <q-page
    ref="chatPageRef"
    :class="['column no-wrap chat-page', paneClass]"
    :style-fn="pageStyleFn"
  >
    <q-banner v-if="globalStore.zeroError" class="bg-negative text-white" dense rounded>
      <template v-slot:action>
        <q-btn flat dense :label="$t('chat.errorDismiss')" @click="globalStore.zeroError = null" />
      </template>
      {{ globalStore.zeroError }}
    </q-banner>

    <div
      class="col chat-messages-scroll q-pa-md"
      ref="messagesContainer"
      style="padding-bottom: 84px"
      @scroll="onScroll"
    >
      <!-- Loading session history -->
      <div v-if="store.isLoadingSession" class="flex flex-center full-height text-grey-6">
        <div class="text-center" style="width: 100%; max-width: 500px">
          <q-spinner-dots size="40px" color="grey-6" />
          <div class="text-body1 q-mt-md">
            {{
              loadingSessionInfo
                ? $t("chat.loadingSessionTitle", {
                    title: loadingSessionInfo.title || loadingSessionInfo.session_id?.slice(-8),
                  })
                : $t("chat.loadingSession")
            }}
          </div>
          <div v-if="loadingSessionInfo" class="text-caption q-mt-xs text-grey-5">
            {{ loadingSessionInfo.model_id || "" }}
            <template v-if="loadingSessionInfo.model_id && loadingSessionInfo.created_at"
              >&nbsp;&middot;&nbsp;</template
            >
            {{
              loadingSessionInfo.created_at ? formatSessionDate(loadingSessionInfo.created_at) : ""
            }}
          </div>

          <!-- Skeleton placeholders -->
          <div class="skeleton-list q-mt-lg q-px-md">
            <div
              v-for="i in 4"
              :key="'sk-' + i"
              class="skeleton-bar"
              :class="i % 2 === 0 ? 'skeleton-bar--short' : 'skeleton-bar--long'"
              :style="{ animationDelay: `${i * 120}ms` }"
            />
          </div>
        </div>
      </div>

      <!-- Empty state -->
      <div
        v-else-if="store.messages.length === 0 && !store.currentResponse && !store.currentThinking"
        class="flex flex-center full-height text-grey-6 text-center"
      >
        <!-- Session exists but has no messages -->
        <div v-if="store.sessionId">
          <q-icon name="chat_bubble_outline" size="48px" color="grey-5" />
          <div class="text-body1 q-mt-sm">{{ $t("chat.emptySession") }}</div>
          <div class="text-caption q-mt-xs">{{ $t("chat.emptySessionSubtitle") }}</div>
        </div>
        <!-- No session at all: prompt to send first message -->
        <div v-else>
          <img
            :src="$q.dark.isActive ? '/zero-completa.png' : '/zero-completa-white.png'"
            alt="Zero"
            style="width: auto; height: auto; margin-bottom: 8px"
          />
          <div class="text-body1">{{ $t("chat.emptyTitle") }}</div>
          <div class="text-caption">{{ $t("chat.emptySubtitle") }}</div>
        </div>
      </div>

      <div v-for="message in store.messages" :key="message.id">
        <TextMessage v-if="message.type === 'text'" :message="message" />
        <ThinkingBlock v-else-if="message.type === 'thinking'" :message="message" />
        <ToolCallMessage v-else-if="message.type === 'tool_call'" :message="message" />
        <PermissionDecisionBadge
          v-else-if="
            message.type === 'permission_request' &&
            !(message.status === 'pending' && message.answerable)
          "
          :message="permissionDecisionBadgeFrom(message)"
        />
        <PermissionDecisionBadge
          v-else-if="message.type === 'permission_decision'"
          :message="message"
        />
        <ErrorMessage v-else-if="message.type === 'error'" :message="message" />
      </div>

      <ThinkingBlock
        v-if="store.currentThinking"
        :message="{ content: store.currentThinking }"
        :streaming="true"
      />

      <q-chat-message
        v-if="store.currentResponse"
        :text="[renderMarkdown(store.currentResponse)]"
        text-html
        class="md-chat-message chat-bubble-generating"
      />
    </div>

    <div class="chat-view__right" v-if="store.activePlan">
      <PlanPanel :plan="store.activePlan" />
    </div>

    <div :class="['chat-input-bar q-pa-sm', $q.dark.isActive ? 'chat-input-bar--dark' : '']">
      <PendingPermissionPanel v-if="pendingPermission" :request="pendingPermission" />
      <ChatInput
        v-model="store.draftText"
        :placeholder="$t('chat.placeholder')"
        :disabled="!canSend"
        :loading="store.runInProgress"
        :working-status="store.workingStatus"
        :plan="store.activePlan"
        @send="onSend"
        @cancel="store.cancelRun()"
        @focus="$emit('focus-input')"
      />
    </div>
  </q-page>
</template>

<script setup>
import { ref, computed, watch, nextTick, onMounted, onUnmounted, provide, inject } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";
import { useZeroSessionStore } from "@/stores/zero-session-store";
import { renderMarkdown } from "@/utils/markdown";
import TextMessage from "@/components/chat/TextMessage.vue";
import ThinkingBlock from "@/components/chat/ThinkingBlock.vue";
import ToolCallMessage from "@/components/chat/ToolCallMessage.vue";
import PermissionDecisionBadge from "@/components/chat/PermissionDecisionBadge.vue";
import PendingPermissionPanel from "@/components/chat/PendingPermissionPanel.vue";
import ErrorMessage from "@/components/chat/ErrorMessage.vue";
import ChatInput from "@/components/chat/ChatInput.vue";
import PlanPanel from "@/components/chat/PlanPanel.vue";

const PANE_NARROW_THRESHOLD = 500;

function permissionDecisionBadgeFrom(request) {
  // A recorded decision (status set from a live answer or a replayed
  // permission_decision history entry) always wins - only a request that's
  // genuinely still unanswered and no longer answerable counts as expired.
  let action;
  if (request.status === "denied") {
    action = "deny";
  } else if (request.status === "approved") {
    action = "allow";
  } else if (request.answerable === false) {
    action = "expired";
  } else {
    action = "allow";
  }
  return {
    toolName: request.toolName,
    action,
    reason: request.reason,
    riskLevel: request.riskLevel,
  };
}

const props = defineProps({
  sessionKey: {
    type: String,
    required: true,
  },
});

defineEmits(["focus-input"]);

const $q = useQuasar();
const store = useZeroSessionStore(props.sessionKey);
provide("zeroStore", store);
const globalStore = useZeroStore();
const { locale } = useI18n();
const messagesContainer = ref(null);
const chatPageRef = ref(null);
const paneWidth = ref(9999);
const isUserAtBottom = ref(true);

provide("paneWidth", paneWidth);

const paneClass = computed(() =>
  paneWidth.value < PANE_NARROW_THRESHOLD ? "pane--narrow" : "pane--regular",
);

const pendingPermission = computed(() =>
  store.messages.find(
    (m) => m.type === "permission_request" && m.status === "pending" && m.answerable,
  ),
);

const loadingSessionInfo = computed(() => {
  if (!store.sessionId) return null;
  return { session_id: store.sessionId };
});

const canSend = computed(
  () =>
    globalStore.hasZero && !store.isConnecting && !store.runInProgress && !pendingPermission.value,
);

// Quasar's QPage defaults to `min-height`, which lets content grow past the
// viewport and makes the whole window scroll instead of just the message
// list. Returning an explicit `height` here (computed from Quasar's own
// tracked screen height, not CSS percentages) guarantees the page is bounded
// so the input bar (position: absolute, see .chat-input-bar) stays pinned
// to its bottom instead of drifting with page-level scroll.
function pageStyleFn(offset, height) {
  return { height: `${height - offset}px` };
}

function formatSessionDate(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  const currentLocale = locale.value;
  return (
    d.toLocaleDateString(currentLocale, { day: "2-digit", month: "2-digit", year: "2-digit" }) +
    " " +
    d.toLocaleTimeString(currentLocale, { hour: "2-digit", minute: "2-digit" })
  );
}

function scrollToBottomIfNeeded() {
  nextTick(() => {
    if (!messagesContainer.value) return;
    const container = messagesContainer.value;
    if (isUserAtBottom.value) {
      container.scrollTop = container.scrollHeight;
    }
  });
}

function onScroll() {
  if (!messagesContainer.value) return;
  const container = messagesContainer.value;
  const threshold = 24;
  isUserAtBottom.value =
    container.scrollHeight - container.scrollTop - container.clientHeight <= threshold;
}

watch(
  [() => store.messages.length, () => store.currentResponse, () => store.currentThinking],
  () => {
    scrollToBottomIfNeeded();
  },
);

async function onSend({ content, file }) {
  if (!canSend.value || (!content && !file)) return;

  store.draftText = "";
  await store.sendMessage(content, file);
}

function onKeydown(event) {
  if (event.key === "Escape" && store.runInProgress) {
    event.preventDefault();
    store.cancelRun();
  }
}

let resizeObserver = null;

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  const el = chatPageRef.value?.$el || chatPageRef.value;
  if (el && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        paneWidth.value = entry.contentRect.width;
      }
    });
    resizeObserver.observe(el);
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  if (resizeObserver) {
    resizeObserver.disconnect();
    resizeObserver = null;
  }
});
</script>

<style scoped>
.chat-page {
  position: relative;
  overflow: hidden;
}

.chat-messages-scroll {
  min-height: 0;
  overflow-y: auto;
}

/* Matches ChatInput's --status-writing accent, since both signal "zero is
   actively producing this response" using the same color language. */
.chat-bubble-generating :deep(.q-message-text--received) {
  color: rgba(33, 186, 69, 0.1);
  border: 1px solid rgba(33, 186, 69, 0.35);
}

.chat-input-bar {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 10;
  background: linear-gradient(
    to bottom,
    rgba(245, 245, 245, 0) 0,
    rgba(245, 245, 245, 0.92) 16px,
    rgba(245, 245, 245, 0.98) 100%
  );
}

.chat-input-bar--dark {
  background: linear-gradient(
    to bottom,
    rgba(18, 18, 18, 0) 0,
    rgba(18, 18, 18, 0.92) 16px,
    rgba(18, 18, 18, 0.98) 100%
  );
}

/* Skeleton loading animation */
.skeleton-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: flex-start;
}

.skeleton-bar {
  height: 14px;
  border-radius: 7px;
  background: linear-gradient(
    90deg,
    rgba(128, 128, 128, 0.12) 25%,
    rgba(128, 128, 128, 0.22) 50%,
    rgba(128, 128, 128, 0.12) 75%
  );
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
}

.skeleton-bar--long {
  width: 80%;
}

.skeleton-bar--short {
  width: 50%;
  align-self: flex-end;
}

@keyframes skeleton-shimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

.chat-view__right {
  width: 260px;
  flex-shrink: 0;
  border-left: 1px solid var(--chat-card-border, rgba(128, 128, 128, 0.18));
  background: rgba(128, 128, 128, 0.04);
}

@media (max-width: 1024px) {
  .chat-view__right {
    display: none;
  }
}

.pane--narrow .chat-view__right {
  display: none;
}

.pane--narrow .chat-input-bar {
  padding: 2px;
}

.pane--narrow .chat-messages-scroll {
  padding: 4px 8px;
}

.pane--narrow :deep(.tool-call-card) {
  font-size: 0.9em;
}
</style>
