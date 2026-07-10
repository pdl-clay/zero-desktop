<template>
  <q-chat-message
    :text="[renderedText]"
    :text-html="isMarkdown"
    :sent="message.role === 'user'"
    :bg-color="bubbleColor"
    :class="[isMarkdown ? 'md-chat-message' : '', bubbleClass]"
  />
</template>

<script setup>
import { computed } from "vue";
import { renderMarkdown } from "@/utils/markdown";

const props = defineProps({
  message: { type: Object, required: true },
});

const isMarkdown = computed(() => props.message.role !== "user");

const renderedText = computed(() =>
  isMarkdown.value ? renderMarkdown(props.message.content) : props.message.content,
);

// Backgrounds for user/assistant are handled entirely via the .chat-bubble-*
// classes below (translucent, theme-aware) instead of Quasar's bg-color
// prop, which only accepts flat palette colors - that's what made the sent
// bubble a solid opaque green instead of following the rest of ChatView's
// subtle-tint card language (ToolCallMessage, ChatInput, response bubble).
const bubbleColor = computed(() => (props.message.role === "system" ? "info" : undefined));

const bubbleClass = computed(() => {
  switch (props.message.role) {
    case "user":
      return "chat-bubble-sent";
    case "assistant":
      return "chat-bubble-response";
    default:
      return "";
  }
});
</script>

<style scoped>
.chat-bubble-response :deep(.q-message-text--received) {
  color: var(--chat-card-bg);
  border: 1px solid var(--chat-card-border);
}

.chat-bubble-sent :deep(.q-message-text--sent) {
  color: rgba(25, 210, 77, 0.14);
  border: 1px solid rgba(25, 210, 77, 0.4);
}

.chat-bubble-sent :deep(.q-message-text-content--sent) {
  color: var(--chat-text);
}
</style>
