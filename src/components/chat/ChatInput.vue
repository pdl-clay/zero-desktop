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
    <div v-if="statusLabel" class="chat-input__status">
      <span class="chat-input__status-dot" />
      <span class="chat-input__status-label">{{ statusLabel }}</span>
    </div>
    <div class="chat-input__row">
      <textarea
        ref="textareaRef"
        v-model="localValue"
        class="chat-input__textarea"
        :placeholder="placeholder"
        :disabled="disabled"
        rows="1"
        @keydown.enter="onEnterKey"
        @input="autoResize"
        @focus="focused = true"
        @blur="focused = false"
      />
      <button
        type="button"
        class="chat-input__send"
        :class="{ 'chat-input__send--active': canSubmit }"
        :disabled="!canSubmit"
        @click="submit"
      >
        <q-spinner-dots v-if="loading" size="16px" color="white" />
        <q-icon v-else name="arrow_upward" size="20px" />
      </button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, nextTick, watch } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";

const props = defineProps({
  modelValue: { type: String, default: "" },
  placeholder: { type: String, default: "" },
  disabled: { type: Boolean, default: false },
  loading: { type: Boolean, default: false },
  workingStatus: { type: [Object, String], default: null },
});

const emit = defineEmits(["update:modelValue", "send"]);

const $q = useQuasar();
const { t } = useI18n();
const textareaRef = ref(null);
const focused = ref(false);

const localValue = computed({
  get: () => props.modelValue,
  set: (value) => emit("update:modelValue", value),
});

const canSubmit = computed(() => !props.disabled && props.modelValue.trim().length > 0);

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

function submit() {
  if (!canSubmit.value) return;
  emit("send", props.modelValue.trim());
  nextTick(autoResize);
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

.chat-input--disabled {
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
</style>
