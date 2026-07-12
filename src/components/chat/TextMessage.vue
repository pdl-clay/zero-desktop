<template>
  <div class="text-message" :class="message.role === 'user' ? 'text-message--sent' : ''">
    <img
      v-if="message.image"
      :src="imageSrc"
      :alt="message.image.name"
      class="text-message__thumb"
    />
    <q-chat-message
      v-if="message.content"
      :text="[renderedText]"
      :text-html="isMarkdown"
      :sent="message.role === 'user'"
      :bg-color="bubbleColor"
      :class="[isMarkdown ? 'md-chat-message' : '', bubbleClass]"
    />
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount } from "vue";
import { renderMarkdown } from "@/utils/markdown";
import { base64ToObjectUrl, base64ToDataUri } from "@/utils/image";

const props = defineProps({
  message: { type: Object, required: true },
});

const isMarkdown = computed(() => props.message.role !== "user");

const renderedText = computed(() =>
  isMarkdown.value ? renderMarkdown(props.message.content) : props.message.content,
);

// message.image is set once when the message is created and never mutated
// afterward, and this component is keyed by message.id (one instance per
// message - see ChatView.vue), so the object URL only ever needs creating
// once and revoking when this instance goes away.
let createdImageUrl = null;
const imageSrc = computed(() => {
  if (!props.message.image) return null;
  if (!createdImageUrl) {
    try {
      createdImageUrl = base64ToObjectUrl(props.message.image.data, props.message.image.mimeType);
    } catch {
      createdImageUrl = base64ToDataUri(props.message.image.data, props.message.image.mimeType);
    }
  }
  return createdImageUrl;
});

onBeforeUnmount(() => {
  if (createdImageUrl) URL.revokeObjectURL(createdImageUrl);
});

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
.text-message {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.text-message--sent {
  align-items: flex-end;
}

.text-message__thumb {
  max-width: 220px;
  max-height: 220px;
  border-radius: 12px;
  object-fit: cover;
  border: 1px solid var(--chat-card-border);
}

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
