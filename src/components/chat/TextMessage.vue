<template>
  <q-chat-message
    :text="[renderedText]"
    :text-html="isMarkdown"
    :sent="message.role === 'user'"
    :bg-color="bubbleColor"
    :text-color="message.role === 'user' ? 'white' : undefined"
    :class="[isMarkdown ? 'md-chat-message' : '', message.role === 'assistant' ? 'chat-bubble-response' : '']"
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

const bubbleColor = computed(() => {
  switch (props.message.role) {
    case "user":
      return "primary";
    case "system":
      return "info";
    case "assistant":
      // Background handled by .chat-bubble-response (see style block) so it
      // matches the neutral tone shared with ChatInput/ToolCallMessage
      // instead of Quasar's flat grey-9/grey-3 palette colors.
      return undefined;
    default:
      return "warning";
  }
});
</script>

<style scoped>
.chat-bubble-response :deep(.q-message-text--received) {
  color: var(--chat-card-bg);
  border: 1px solid var(--chat-card-border);
}
</style>
