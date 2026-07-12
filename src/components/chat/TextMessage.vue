<template>
  <div class="text-message" :class="message.role === 'user' ? 'text-message--sent' : ''">
    <img v-if="isImageAttachment" :src="imageSrc" :alt="fileName" class="text-message__thumb" />
    <div v-else-if="isTextAttachment" class="text-message__file-chip row items-center">
      <q-icon :name="fileIcon" size="22px" class="q-mr-sm" />
      <div class="text-message__file-info column">
        <span class="text-message__file-name">{{ fileName }}</span>
        <span class="text-message__file-meta">{{ fileMime }}</span>
      </div>
    </div>
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
import { isImageMimeType, isTextMimeType, getFileIcon } from "@/utils/file";

const props = defineProps({
  message: { type: Object, required: true },
});

const file = computed(() => props.message.file || props.message.image || null);
const isImageAttachment = computed(() => file.value && isImageMimeType(file.value.mimeType));
const isTextAttachment = computed(() => file.value && isTextMimeType(file.value.mimeType));
const fileName = computed(() => file.value?.name || "");
const fileMime = computed(() => file.value?.mimeType || "");
const fileIcon = computed(() => getFileIcon(fileMime.value, fileName.value));

const isMarkdown = computed(() => props.message.role !== "user");

const renderedText = computed(() =>
  isMarkdown.value ? renderMarkdown(props.message.content) : props.message.content,
);

// message.file is set once when the message is created and never mutated
// afterward, and this component is keyed by message.id (one instance per
// message - see ChatView.vue), so the object URL only ever needs creating
// once and revoking when this instance goes away.
let createdImageUrl = null;
const imageSrc = computed(() => {
  if (!isImageAttachment.value) return null;
  if (!createdImageUrl) {
    try {
      createdImageUrl = base64ToObjectUrl(file.value.data, file.value.mimeType);
    } catch {
      createdImageUrl = base64ToDataUri(file.value.data, file.value.mimeType);
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

.text-message__file-chip {
  padding: 8px 12px;
  border-radius: 12px;
  background: rgba(128, 128, 128, 0.1);
  border: 1px solid var(--chat-card-border);
  color: var(--chat-text);
  max-width: 300px;
}

.text-message__file-info {
  min-width: 0;
}

.text-message__file-name {
  font-size: 0.88em;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 220px;
}

.text-message__file-meta {
  font-size: 0.75em;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 220px;
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
