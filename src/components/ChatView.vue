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
    >
      <div
        v-if="
          zeroStore.messages.length === 0 &&
          !zeroStore.currentResponse &&
          !zeroStore.currentThinking
        "
        class="flex flex-center full-height text-grey-6 text-center"
      >
        <div>
          <img
            src="/zero-completa.png"
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
        <PermissionRequest v-else-if="message.type === 'permission_request'" :message="message" />
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
      <ChatInput
        v-model="input"
        :placeholder="$t('chat.placeholder')"
        :disabled="!canSend"
        :loading="zeroStore.runInProgress"
        :working-status="zeroStore.workingStatus"
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
import PermissionRequest from "@/components/chat/PermissionRequest.vue";
import PermissionDecisionBadge from "@/components/chat/PermissionDecisionBadge.vue";
import ErrorMessage from "@/components/chat/ErrorMessage.vue";
import WorkingIndicator from "@/components/chat/WorkingIndicator.vue";
import ChatInput from "@/components/chat/ChatInput.vue";

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

const canSend = computed(() => zeroStore.isConnected && !zeroStore.runInProgress);

// Quasar's QPage defaults to `min-height`, which lets content grow past the
// viewport and makes the whole window scroll instead of just the message
// list. Returning an explicit `height` here (computed from Quasar's own
// tracked screen height, not CSS percentages) guarantees the page is bounded
// so the input bar (position: absolute, see .chat-input-bar) stays pinned
// to its bottom instead of drifting with page-level scroll.
function pageStyleFn(offset, height) {
  return { height: `${height - offset}px` };
}

watch(
  [
    () => zeroStore.messages.length,
    () => zeroStore.currentResponse,
    () => zeroStore.currentThinking,
  ],
  () => {
    nextTick(() => {
      if (messagesContainer.value) {
        messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight;
      }
    });
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
</style>
