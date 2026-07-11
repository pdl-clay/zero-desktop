<template>
  <q-page class="column no-wrap chat-page" :style-fn="pageStyleFn">
    <q-banner v-if="zeroStore.zeroError" class="bg-negative text-white" dense rounded>
      <template v-slot:action>
        <q-btn flat dense label="OK" @click="zeroStore.zeroError = null" />
      </template>
      {{ zeroStore.zeroError }}
    </q-banner>

    <WorkingIndicator />

    <div
      class="col chat-messages-scroll q-pa-md"
      ref="messagesContainer"
      style="padding-bottom: 84px"
      @scroll="onScroll"
    >
      <!-- Loading session history -->
      <div
        v-if="zeroStore.isLoadingSession"
        class="flex flex-center full-height text-grey-6"
      >
        <div class="text-center" style="width: 100%; max-width: 500px">
          <q-spinner-dots size="40px" color="grey-6" />
          <div class="text-body1 q-mt-md">
            {{
              loadingSessionInfo
                ? $t("chat.loadingSessionTitle", { title: loadingSessionInfo.title || loadingSessionInfo.session_id?.slice(-8) })
                : $t("chat.loadingSession")
            }}
          </div>
          <div v-if="loadingSessionInfo" class="text-caption q-mt-xs text-grey-5">
            {{ loadingSessionInfo.model_id || "" }}
            <template v-if="loadingSessionInfo.model_id && loadingSessionInfo.created_at">&nbsp;&middot;&nbsp;</template>
            {{ loadingSessionInfo.created_at ? formatSessionDate(loadingSessionInfo.created_at) : "" }}
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
        v-else-if="
          zeroStore.messages.length === 0 &&
          !zeroStore.currentResponse &&
          !zeroStore.currentThinking
        "
        class="flex flex-center full-height text-grey-6 text-center"
      >
        <!-- Session exists but has no messages -->
        <div v-if="zeroStore.currentSessionId">
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

      <div v-for="message in zeroStore.messages" :key="message.id">
        <TextMessage v-if="message.type === 'text'" :message="message" />
        <ThinkingBlock v-else-if="message.type === 'thinking'" :message="message" />
        <ToolCallMessage v-else-if="message.type === 'tool_call'" :message="message" />
        <PermissionDecisionBadge
          v-else-if="message.type === 'permission_request'"
          :message="permissionDecisionBadgeFrom(message)"
        />
        <PermissionDecisionBadge
          v-else-if="message.type === 'permission_decision'"
          :message="message"
        />
        <ErrorMessage v-else-if="message.type === 'error'" :message="message" />
      </div>

      <ThinkingBlock
        v-if="zeroStore.currentThinking"
        :message="{ content: zeroStore.currentThinking }"
        :streaming="true"
      />

      <q-chat-message
        v-if="zeroStore.currentResponse"
        :text="[renderMarkdown(zeroStore.currentResponse)]"
        text-html
        class="md-chat-message chat-bubble-generating"
      />
    </div>

    <div :class="['chat-input-bar q-pa-sm', $q.dark.isActive ? 'chat-input-bar--dark' : '']">
      <PendingPermissionPanel v-if="pendingPermission" :request="pendingPermission" />
      <ChatInput
        v-model="input"
        :placeholder="$t('chat.placeholder')"
        :disabled="!canSend"
        :loading="zeroStore.runInProgress"
        :working-status="zeroStore.workingStatus"
        :plan="zeroStore.activePlan"
        @send="onSend"
      />
    </div>
  </q-page>
</template>

<script setup>
import { ref, computed, watch, nextTick } from "vue";
import { useQuasar } from "quasar";
import { useZeroStore } from "@/stores/zero-store";
import { renderMarkdown } from "@/utils/markdown";
import TextMessage from "@/components/chat/TextMessage.vue";
import ThinkingBlock from "@/components/chat/ThinkingBlock.vue";
import ToolCallMessage from "@/components/chat/ToolCallMessage.vue";
import PermissionDecisionBadge from "@/components/chat/PermissionDecisionBadge.vue";
import PendingPermissionPanel from "@/components/chat/PendingPermissionPanel.vue";
import ErrorMessage from "@/components/chat/ErrorMessage.vue";
import WorkingIndicator from "@/components/chat/WorkingIndicator.vue";
import ChatInput from "@/components/chat/ChatInput.vue";

function permissionDecisionBadgeFrom(request) {
  return {
    toolName: request.toolName,
    action: request.status === "denied" ? "deny" : "allow",
    reason: request.reason,
    riskLevel: request.riskLevel,
  };
}

const props = defineProps({
  workspacePath: {
    type: String,
    required: true,
  },
});

const $q = useQuasar();
const zeroStore = useZeroStore();
const input = ref("");
const messagesContainer = ref(null);
const isUserAtBottom = ref(true);

const pendingPermission = computed(() =>
  zeroStore.messages.find((m) => m.type === "permission_request" && m.status === "pending"),
);

const loadingSessionInfo = computed(() => {
  if (!zeroStore.currentSessionId) return null;
  return zeroStore.sessions.find((s) => s.session_id === zeroStore.currentSessionId) || null;
});

const canSend = computed(
  () => zeroStore.isConnected && !zeroStore.runInProgress && !pendingPermission.value,
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
  return (
    d.toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit", year: "2-digit" }) +
    " " +
    d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })
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
  [
    () => zeroStore.messages.length,
    () => zeroStore.currentResponse,
    () => zeroStore.currentThinking,
  ],
  () => {
    scrollToBottomIfNeeded();
  },
);

async function onSend(content) {
  if (!canSend.value || !content) return;

  input.value = "";
  await zeroStore.sendMessage(content);
}
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
</style>
