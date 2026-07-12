<template>
  <div
    :class="[
      'chat-input',
      $q.dark.isActive ? 'chat-input--dark' : '',
      focused ? 'chat-input--focused' : '',
      disabled ? 'chat-input--disabled' : '',
      statusClass,
    ]"
  >
    <div v-if="plan && plan.length > 0" class="chat-input__plan">
      <div v-for="(item, i) in plan" :key="i" class="chat-input__plan-item row items-start">
        <q-icon
          :name="planIcon(item.status)"
          :color="planColor(item.status)"
          size="14px"
          class="q-mr-xs q-mt-xs"
        />
        <span
          :class="[
            'chat-input__plan-text',
            item.status === 'completed' ? 'chat-input__plan-text--done' : '',
          ]"
        >
          {{ item.content }}
        </span>
      </div>
    </div>

    <div v-if="statusLabel" class="chat-input__status">
      <span class="chat-input__status-dot" />
      <span class="chat-input__status-label">{{ statusLabel }}</span>
    </div>
    <div v-if="attachedImage" class="chat-input__attachment">
      <div class="chat-input__thumb-wrap">
        <img :src="attachedImage.previewUrl" :alt="attachedImage.name" class="chat-input__thumb" />
        <button type="button" class="chat-input__thumb-remove" @click="removeAttachedImage">
          <q-icon name="close" size="12px" />
          <q-tooltip>{{ t("chat.removeImage") }}</q-tooltip>
        </button>
      </div>
    </div>
    <div class="chat-input__row">
      <button
        type="button"
        class="chat-input__attach"
        :disabled="disabled || pickingImage"
        @click="pickImage"
      >
        <q-icon name="attach_file" size="18px" />
        <q-tooltip>{{ t("chat.attachImage") }}</q-tooltip>
      </button>
      <textarea
        ref="textareaRef"
        v-model="localValue"
        class="chat-input__textarea"
        :placeholder="placeholder"
        :disabled="disabled"
        rows="1"
        @keydown.enter="onEnterKey"
        @input="autoResize"
        @focus="onFocus"
        @blur="focused = false"
      />
      <button
        type="button"
        class="chat-input__send"
        :class="{
          'chat-input__send--active': canSubmit && !loading,
          'chat-input__send--cancel': loading,
        }"
        :disabled="!loading && !canSubmit"
        @click="loading ? onCancel() : submit()"
      >
        <q-icon v-if="loading" name="pause" size="18px" />
        <q-icon v-else name="arrow_upward" size="20px" />
        <q-tooltip v-if="loading">{{ t("chat.cancelRun") }}</q-tooltip>
      </button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, nextTick, watch, onBeforeUnmount } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { planIcon, planColor } from "@/utils/plan";
import { readImageAttachment } from "@/services/zero";
import { base64ToObjectUrl, base64ToDataUri } from "@/utils/image";
import { useZeroStore } from "@/stores/zero-store";

const props = defineProps({
  modelValue: { type: String, default: "" },
  placeholder: { type: String, default: "" },
  disabled: { type: Boolean, default: false },
  loading: { type: Boolean, default: false },
  workingStatus: { type: [Object, String], default: null },
  plan: { type: Array, default: null },
});

const emit = defineEmits(["update:modelValue", "send", "cancel", "focus"]);

const $q = useQuasar();
const { t } = useI18n();
const zeroStore = useZeroStore();
const textareaRef = ref(null);
const focused = ref(false);
const attachedImage = ref(null);
const pickingImage = ref(false);

const localValue = computed({
  get: () => props.modelValue,
  set: (value) => emit("update:modelValue", value),
});

const canSubmit = computed(
  () => !props.disabled && (props.modelValue.trim().length > 0 || !!attachedImage.value),
);

const statusLabel = computed(() => {
  const status = props.workingStatus;
  if (!status || !props.disabled) return null;
  if (status === "thinking") return t("chat.thinkingRunning");
  if (status === "writing") return t("chat.writing");
  if (status === "sending") return t("chat.sending");
  if (typeof status === "object" && status.type === "tool") {
    return `${status.toolName} ${t("chat.toolRunning")}`;
  }
  return null;
});

const statusClass = computed(() => {
  const status = props.workingStatus;
  if (!status || !props.disabled) return "";
  if (status === "thinking") return "chat-input--status-thinking";
  if (status === "writing") return "chat-input--status-writing";
  if (status === "sending") return "chat-input--status-sending";
  if (typeof status === "object" && status.type === "tool") return "chat-input--status-tool";
  return "";
});

const MAX_HEIGHT = 200;

function autoResize() {
  const el = textareaRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, MAX_HEIGHT)}px`;
}

watch(
  () => props.modelValue,
  () => nextTick(autoResize),
);

function onEnterKey(event) {
  if (event.shiftKey) return;
  event.preventDefault();
  submit();
}

function releaseAttachedImagePreview() {
  if (attachedImage.value?.previewUrl) {
    URL.revokeObjectURL(attachedImage.value.previewUrl);
  }
}

function submit() {
  if (!canSubmit.value) return;
  emit("send", {
    content: props.modelValue.trim(),
    image: attachedImage.value
      ? {
          mimeType: attachedImage.value.mimeType,
          data: attachedImage.value.data,
          name: attachedImage.value.name,
        }
      : null,
  });
  releaseAttachedImagePreview();
  attachedImage.value = null;
  nextTick(autoResize);
}

async function pickImage() {
  const selected = await open({
    multiple: false,
    title: t("chat.attachImageTitle"),
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }],
  });
  if (!selected) return;

  pickingImage.value = true;
  try {
    const attachment = await readImageAttachment(selected);
    releaseAttachedImagePreview();
    let previewUrl;
    try {
      previewUrl = base64ToObjectUrl(attachment.data, attachment.mimeType);
    } catch {
      previewUrl = base64ToDataUri(attachment.data, attachment.mimeType);
    }
    attachedImage.value = {
      ...attachment,
      previewUrl,
    };
  } catch (error) {
    zeroStore.zeroError = error;
  } finally {
    pickingImage.value = false;
  }
}

function removeAttachedImage() {
  releaseAttachedImagePreview();
  attachedImage.value = null;
}

onBeforeUnmount(releaseAttachedImagePreview);

function onCancel() {
  emit("cancel");
}

function onFocus() {
  focused.value = true;
  emit("focus");
}

defineExpose({ focus: () => textareaRef.value?.focus() });
</script>

<style scoped>
.chat-input {
  display: flex;
  flex-direction: column;
  border-radius: 22px;
  background: rgba(128, 128, 128, 0.09);
  border: 1px solid rgba(128, 128, 128, 0.16);
  transition:
    border-color 0.15s ease,
    background 0.15s ease;
  overflow: hidden;
}

.chat-input__row {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 6px 6px 6px 16px;
}

.chat-input__plan {
  padding: 8px 16px 4px;
  border-bottom: 1px solid var(--chat-card-border);
  max-height: 140px;
  overflow-y: auto;
}

.chat-input__plan-item {
  padding: 2px 0;
}

.chat-input__plan-text {
  font-size: 0.82em;
  line-height: 1.4;
  color: var(--chat-text);
}

.chat-input__plan-text--done {
  color: var(--chat-text-muted);
  text-decoration: line-through;
}

.chat-input__status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  font-size: 0.8em;
  overflow: hidden;
  position: relative;
}

.chat-input__status::after {
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 255, 255, 0.06) 50%,
    transparent 100%
  );
  animation: chat-input-shimmer 2s ease-in-out infinite;
}

.chat-input__status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  animation: chat-input-pulse 1.5s ease-in-out infinite;
}

.chat-input__status-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.3;
}

.chat-input__attachment {
  padding: 10px 16px 0;
}

.chat-input__thumb-wrap {
  position: relative;
  display: inline-flex;
  width: 48px;
  height: 48px;
}

.chat-input__thumb {
  width: 48px;
  height: 48px;
  border-radius: 10px;
  object-fit: cover;
  border: 1px solid rgba(128, 128, 128, 0.25);
}

.chat-input__thumb-remove {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(244, 67, 54, 0.14);
  color: #f44336;
  cursor: pointer;
  transition:
    background 0.15s ease,
    transform 0.1s ease;
}

.chat-input__thumb-remove:hover {
  background: rgba(244, 67, 54, 0.24);
  transform: scale(1.04);
}

.chat-input__thumb-remove:active {
  transform: scale(0.96);
}

@keyframes chat-input-pulse {
  0%,
  100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.5;
    transform: scale(0.85);
  }
}

@keyframes chat-input-shimmer {
  0%,
  100% {
    transform: translateX(-100%);
  }
  50% {
    transform: translateX(100%);
  }
}

.chat-input--status-thinking {
  border-color: rgba(242, 192, 55, 0.35);
}

.chat-input--status-thinking .chat-input__status {
  background: rgba(242, 192, 55, 0.1);
  color: #f2c037;
}

.chat-input--status-thinking .chat-input__status-dot {
  background: #f2c037;
}

.chat-input--status-tool {
  border-color: rgba(49, 204, 236, 0.35);
}

.chat-input--status-tool .chat-input__status {
  background: rgba(49, 204, 236, 0.1);
  color: #31ccec;
}

.chat-input--status-tool .chat-input__status-dot {
  background: #31ccec;
}

.chat-input--status-writing {
  border-color: rgba(33, 186, 69, 0.35);
}

.chat-input--status-writing .chat-input__status {
  background: rgba(33, 186, 69, 0.1);
  color: #21ba45;
}

.chat-input--status-writing .chat-input__status-dot {
  background: #21ba45;
}

.chat-input--status-sending {
  border-color: rgba(128, 128, 128, 0.35);
}

.chat-input--status-sending .chat-input__status {
  background: rgba(128, 128, 128, 0.08);
  color: #909090;
}

.chat-input--status-sending .chat-input__status-dot {
  background: #909090;
}

.chat-input--dark {
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.1);
}

.chat-input--focused {
  border-color: var(--q-primary, #1976d2);
  background: rgba(128, 128, 128, 0.04);
}

.chat-input--dark.chat-input--focused {
  background: rgba(255, 255, 255, 0.07);
}

.chat-input--disabled .chat-input__textarea {
  opacity: 0.6;
}

.chat-input__textarea {
  flex: 1;
  resize: none;
  border: none;
  outline: none;
  background: transparent;
  font-family: inherit;
  font-size: 0.95em;
  line-height: 1.4;
  max-height: 200px;
  overflow-y: auto;
  padding: 6px 0;
  color: var(--chat-text);
}

.chat-input__textarea::placeholder {
  color: rgba(128, 128, 128, 0.8);
}

.chat-input__attach {
  flex-shrink: 0;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(128, 128, 128, 0.14);
  color: rgba(128, 128, 128, 0.9);
  cursor: pointer;
  transition:
    background 0.15s ease,
    transform 0.1s ease;
}

.chat-input__attach:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.24);
  transform: scale(1.06);
}

.chat-input__attach:active:not(:disabled) {
  transform: scale(0.94);
}

.chat-input__attach:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-input__send {
  flex-shrink: 0;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(128, 128, 128, 0.25);
  color: rgba(128, 128, 128, 0.9);
  cursor: not-allowed;
  transition:
    background 0.15s ease,
    transform 0.1s ease;
}

.chat-input__send--active {
  background: var(--q-primary, #1976d2);
  color: white;
  cursor: pointer;
}

.chat-input__send--active:hover {
  transform: scale(1.06);
}

.chat-input__send--active:active {
  transform: scale(0.94);
}

.chat-input__send:disabled {
  cursor: not-allowed;
}

.chat-input__send--cancel {
  background: rgba(244, 67, 54, 0.16);
  color: #f44336;
  cursor: pointer;
}

.chat-input__send--cancel:hover {
  background: rgba(244, 67, 54, 0.26);
  transform: scale(1.06);
}

.chat-input__send--cancel:active {
  transform: scale(0.94);
}
</style>
