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
    <div v-if="attachedFile" class="chat-input__attachment">
      <div class="chat-input__file-wrap">
        <img
          v-if="attachedFile.previewUrl"
          :src="attachedFile.previewUrl"
          :alt="attachedFile.name"
          class="chat-input__thumb"
        />
        <div v-else class="chat-input__file-chip row items-center">
          <q-icon :name="fileIcon" size="20px" class="q-mr-sm" />
          <div class="chat-input__file-info column">
            <span class="chat-input__file-name">{{ attachedFile.name }}</span>
            <span class="chat-input__file-meta">{{ attachedFile.mimeType }}</span>
          </div>
        </div>
        <button type="button" class="chat-input__thumb-remove" @click="removeAttachedFile">
          <q-icon name="close" size="12px" />
          <q-tooltip>{{ t("chat.removeAttachment") }}</q-tooltip>
        </button>
      </div>
    </div>
    <div class="chat-input__row">
      <button
        type="button"
        class="chat-input__attach"
        :disabled="disabled || pickingFile"
        @click="pickFile"
      >
        <q-icon name="attach_file" size="18px" />
        <q-tooltip>{{ t("chat.attachFile") }}</q-tooltip>
      </button>
      <button
        type="button"
        class="chat-input__mode"
        :class="{
          'chat-input__mode--active': zeroStore.permissionMode === 'auto_allow',
          'chat-input__mode--collapsed': isNarrowViewport,
        }"
        :disabled="disabled"
        @click="togglePermissionMode"
      >
        <q-icon
          :name="zeroStore.permissionMode === 'auto_allow' ? 'check_circle' : 'help_outline'"
          size="14px"
        />
        <span class="chat-input__mode-label">{{ permissionModeLabel }}</span>
      </button>
      <div class="chat-input__model-wrap">
        <button
          type="button"
          class="chat-input__model"
          :class="{
            'chat-input__model--active': effectiveActiveModel,
            'chat-input__model--collapsed': isNarrowViewport,
          }"
          :disabled="disabled"
          @click="toggleModelMenu"
        >
          <q-icon name="memory" size="14px" />
          <span class="chat-input__model-label">{{
            effectiveActiveModel || t("chat.modelLabel")
          }}</span>
          <q-icon
            name="expand_more"
            size="14px"
            class="chat-input__model-chevron"
            :class="{ 'chat-input__model-chevron--open': modelMenuOpen }"
          />
        </button>
        <transition name="chat-input__model-fade">
          <div
            v-if="modelMenuOpen"
            v-click-outside="closeModelMenu"
            class="chat-input__model-dropdown"
          >
            <div class="chat-input__model-header row items-center justify-between">
              <span>{{ t("chat.switchModel") }}</span>
              <q-icon name="memory" size="14px" color="grey-6" />
            </div>
            <div class="chat-input__model-separator" />
            <div class="chat-input__model-search">
              <q-icon name="search" size="14px" color="grey-6" class="q-mr-sm" />
              <input
                v-model="modelSearch"
                type="text"
                :placeholder="t('chat.searchModel')"
                class="chat-input__model-search-input"
                @click.stop
              />
            </div>
            <div v-if="recentModels.length && !modelSearch" class="chat-input__model-section">
              <div class="chat-input__model-section-title">
                {{ t("chat.recentModels") }}
              </div>
              <ul class="chat-input__model-list chat-input__model-list--recent">
                <li
                  v-for="m in recentModels"
                  :key="`recent-${m}`"
                  :class="[
                    'chat-input__model-item',
                    { 'chat-input__model-item--active': m === effectiveActiveModel },
                  ]"
                  @click="selectModel(m)"
                >
                  <span class="chat-input__model-item-avatar">
                    <q-icon
                      v-if="m === effectiveActiveModel"
                      name="check_circle"
                      size="18px"
                      color="primary"
                    />
                    <q-icon v-else name="history" size="18px" color="grey-6" />
                  </span>
                  <span class="chat-input__model-name">{{ m }}</span>
                </li>
              </ul>
              <div class="chat-input__model-separator" />
            </div>
            <ul ref="modelListRef" class="chat-input__model-list">
              <li
                v-for="m in filteredModels"
                :key="m"
                :class="[
                  'chat-input__model-item',
                  { 'chat-input__model-item--active': m === effectiveActiveModel },
                ]"
                @click="selectModel(m)"
              >
                <span class="chat-input__model-item-avatar">
                  <q-icon
                    v-if="m === effectiveActiveModel"
                    name="check_circle"
                    size="18px"
                    color="primary"
                  />
                  <q-icon v-else name="radio_button_unchecked" size="18px" color="grey-6" />
                </span>
                <span class="chat-input__model-name">{{ m }}</span>
              </li>
              <li v-if="zeroStore.isLoadingModels" class="chat-input__model-status">
                <span class="chat-input__model-item-avatar">
                  <q-spinner-dots size="18px" color="primary" />
                </span>
                <span>{{ t("chat.loadingModels") }}</span>
              </li>
              <li v-else-if="filteredModels.length === 0" class="chat-input__model-status">
                <span class="chat-input__model-item-avatar">
                  <q-icon name="search_off" size="18px" color="grey-6" />
                </span>
                <span>{{ t("chat.noModelsMatch") }}</span>
              </li>
            </ul>
          </div>
        </transition>
      </div>
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
import { ref, computed, nextTick, watch, onMounted, onBeforeUnmount, inject } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { planIcon, planColor } from "@/utils/plan";
import { readFileAttachment } from "@/services/zero";
import { base64ToObjectUrl, base64ToDataUri } from "@/utils/image";
import { isImageMimeType, isTextMimeType, getFileIcon } from "@/utils/file";
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
const sessionStore = inject("zeroStore");
const textareaRef = ref(null);
const focused = ref(false);
const attachedFile = ref(null);
const pickingFile = ref(false);
const modelMenuOpen = ref(false);
const modelSearch = ref("");
const modelListRef = ref(null);

const MAX_RECENT_MODELS = 3;
const recentModelsKey = "zero-recent-models";
const permissionModeKey = "zero-permission-mode";

const paneWidth = inject("paneWidth", ref(9999));
const isNarrowViewport = computed(() => paneWidth.value < 500);

const permissionModeLabel = computed(() =>
  zeroStore.permissionMode === "auto_allow" ? t("chat.autoAllow") : t("chat.ask"),
);

const permissionModeTooltip = computed(() =>
  zeroStore.permissionMode === "auto_allow" ? t("chat.autoAllowTooltip") : t("chat.askTooltip"),
);

// The model a session is actually running under is per-session (each
// session's process keeps whatever model it snapshotted when it last
// (re)connected - see zero-session-store.js's switchModel/startSession).
// Falls back to the global store's model only for a panel that hasn't
// connected yet, so it shows the default it would inherit if it did.
const effectiveActiveModel = computed(() => sessionStore.activeModel || zeroStore.activeModel);

const recentModels = computed(() => {
  const active = effectiveActiveModel.value;
  const recent = JSON.parse(localStorage.getItem(recentModelsKey) || "[]").filter(
    (m) => typeof m === "string" && zeroStore.availableModels.includes(m) && m !== active,
  );
  if (active && zeroStore.availableModels.includes(active)) {
    return [active, ...recent.filter((m) => m !== active)].slice(0, MAX_RECENT_MODELS);
  }
  return recent.slice(0, MAX_RECENT_MODELS);
});

// Load eagerly on mount instead of only on first click of the model picker,
// so the menu is already populated by the time the user opens it - the
// @show handler on the q-menu stays as a retry path if this fails silently
// (e.g. a transient network hiccup probing the provider's model list).
onMounted(() => {
  zeroStore.loadAvailableModels();
  const savedMode = localStorage.getItem(permissionModeKey);
  if (savedMode === "ask" || savedMode === "auto_allow") {
    zeroStore.setPermissionMode(savedMode);
  }
});

onBeforeUnmount(() => {
  releaseAttachedFilePreview();
});

const localValue = computed({
  get: () => props.modelValue,
  set: (value) => emit("update:modelValue", value),
});

const canSubmit = computed(
  () => !props.disabled && (props.modelValue.trim().length > 0 || !!attachedFile.value),
);

const fileIcon = computed(() =>
  attachedFile.value
    ? getFileIcon(attachedFile.value.mimeType, attachedFile.value.name)
    : "insert_drive_file",
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

const filteredModels = computed(() => {
  const query = modelSearch.value.trim().toLowerCase();
  const active = effectiveActiveModel.value;
  const recent = recentModels.value;
  let list = zeroStore.availableModels.filter((m) => !recent.includes(m));
  if (query) {
    list = list.filter((m) => m.toLowerCase().includes(query));
  }
  if (active && list.includes(active)) {
    list = [active, ...list.filter((m) => m !== active)];
  }
  return list;
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

function releaseAttachedFilePreview() {
  if (attachedFile.value?.previewUrl) {
    URL.revokeObjectURL(attachedFile.value.previewUrl);
  }
}

function submit() {
  if (!canSubmit.value) return;
  emit("send", {
    content: props.modelValue.trim(),
    file: attachedFile.value
      ? {
          mimeType: attachedFile.value.mimeType,
          data: attachedFile.value.data,
          name: attachedFile.value.name,
        }
      : null,
  });
  releaseAttachedFilePreview();
  attachedFile.value = null;
  nextTick(autoResize);
}

const ATTACHMENT_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "txt",
  "md",
  "csv",
  "json",
  "yaml",
  "yml",
  "xml",
  "html",
  "htm",
  "css",
  "js",
  "ts",
  "jsx",
  "tsx",
  "py",
  "go",
  "rs",
  "java",
  "kt",
  "swift",
  "c",
  "cpp",
  "cc",
  "cxx",
  "h",
  "hpp",
  "rb",
  "php",
  "sh",
  "sql",
  "dockerfile",
];

async function pickFile() {
  const selected = await open({
    multiple: false,
    title: t("chat.attachFileTitle"),
    filters: [
      {
        name: "Supported files",
        extensions: ATTACHMENT_EXTENSIONS,
      },
    ],
  });
  if (!selected) return;

  pickingFile.value = true;
  try {
    const attachment = await readFileAttachment(selected);
    releaseAttachedFilePreview();
    let previewUrl = null;
    if (isImageMimeType(attachment.mimeType)) {
      try {
        previewUrl = base64ToObjectUrl(attachment.data, attachment.mimeType);
      } catch {
        previewUrl = base64ToDataUri(attachment.data, attachment.mimeType);
      }
    }
    attachedFile.value = {
      ...attachment,
      previewUrl,
    };
  } catch (error) {
    zeroStore.zeroError = error;
  } finally {
    pickingFile.value = false;
  }
}

function removeAttachedFile() {
  releaseAttachedFilePreview();
  attachedFile.value = null;
}

function onCancel() {
  emit("cancel");
}

function onFocus() {
  focused.value = true;
  emit("focus");
}

function togglePermissionMode() {
  const nextMode = zeroStore.permissionMode === "ask" ? "auto_allow" : "ask";
  zeroStore.setPermissionMode(nextMode);
  localStorage.setItem(permissionModeKey, nextMode);
}

async function openModelMenu() {
  modelMenuOpen.value = true;
  await zeroStore.loadAvailableModels();
}

function toggleModelMenu() {
  if (modelMenuOpen.value) {
    modelMenuOpen.value = false;
  } else {
    openModelMenu();
  }
}

function closeModelMenu() {
  modelMenuOpen.value = false;
  modelSearch.value = "";
}

function selectModel(model) {
  sessionStore.switchModel(model);
  rememberRecentModel(model);
  closeModelMenu();
}

function rememberRecentModel(model) {
  const recent = JSON.parse(localStorage.getItem(recentModelsKey) || "[]").filter(
    (m) => typeof m === "string" && m !== model,
  );
  recent.unshift(model);
  localStorage.setItem(recentModelsKey, JSON.stringify(recent.slice(0, MAX_RECENT_MODELS)));
}

defineExpose({ focus: () => textareaRef.value?.focus() });

const vClickOutside = {
  mounted(el, binding) {
    el._clickOutside = (event) => {
      if (!(el === event.target || el.contains(event.target))) {
        binding.value();
      }
    };
    document.addEventListener("click", el._clickOutside, true);
  },
  unmounted(el) {
    document.removeEventListener("click", el._clickOutside, true);
  },
};
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
  /* overflow: hidden removido — o dropdown absoluto do seletor de modelos
     precisa ultrapassar os limites do .chat-input */
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

.chat-input__file-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.chat-input__thumb-wrap,
.chat-input__file-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.chat-input__thumb {
  width: 48px;
  height: 48px;
  border-radius: 10px;
  object-fit: cover;
  border: 1px solid rgba(128, 128, 128, 0.25);
}

.chat-input__file-chip {
  padding: 6px 10px;
  border-radius: 10px;
  background: rgba(128, 128, 128, 0.12);
  border: 1px solid rgba(128, 128, 128, 0.2);
  color: var(--chat-text);
  max-width: 260px;
}

.chat-input__file-info {
  min-width: 0;
}

.chat-input__file-name {
  font-size: 0.85em;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 200px;
}

.chat-input__file-meta {
  font-size: 0.72em;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 200px;
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

.chat-input__mode {
  flex-shrink: 0;
  height: 34px;
  width: auto;
  max-width: 140px;
  padding: 0 10px 0 8px;
  border-radius: 17px;
  border: 1px solid rgba(128, 128, 128, 0.22);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background: transparent;
  color: rgba(128, 128, 128, 0.9);
  cursor: pointer;
  font-size: 0.82em;
  font-weight: 500;
  transition:
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease,
    transform 0.1s ease,
    width 0.5s ease,
    max-width 0.5s ease,
    padding 0.5s ease;
}

.chat-input__mode:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.08);
  border-color: rgba(128, 128, 128, 0.32);
}

.chat-input__mode:active:not(:disabled) {
  transform: scale(0.97);
}

.chat-input__mode:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-input__mode--active {
  color: var(--q-primary, #1976d2);
  border-color: rgba(25, 118, 210, 0.35);
  background: rgba(25, 118, 210, 0.06);
}

.chat-input__mode--active:hover:not(:disabled) {
  background: rgba(25, 118, 210, 0.12);
  border-color: rgba(25, 118, 210, 0.45);
}

.chat-input__mode--collapsed {
  width: 34px;
  max-width: 34px;
  padding: 0;
  justify-content: center;
}

.chat-input__mode--collapsed .chat-input__mode-label {
  max-width: 0;
  opacity: 0;
  margin-left: -5px;
}

.chat-input__mode--collapsed:hover:not(:disabled) {
  width: auto;
  max-width: 140px;
  padding: 0 10px 0 8px;
  justify-content: flex-start;
}

.chat-input__mode--collapsed:hover:not(:disabled) .chat-input__mode-label {
  max-width: 90px;
  opacity: 1;
  margin-left: 0;
}

.chat-input__mode-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.3;
  max-width: 90px;
  opacity: 1;
  transition:
    max-width 0.5s ease,
    opacity 0.35s ease,
    margin 0.35s ease;
}

.chat-input__model {
  flex-shrink: 0;
  height: 34px;
  width: auto;
  max-width: 180px;
  padding: 0 8px 0 10px;
  border-radius: 17px;
  border: 1px solid rgba(128, 128, 128, 0.22);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background: transparent;
  color: rgba(128, 128, 128, 0.9);
  cursor: pointer;
  font-size: 0.82em;
  font-weight: 500;
  transition:
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease,
    transform 0.1s ease,
    width 0.5s ease,
    max-width 0.5s ease,
    padding 0.5s ease;
}

.chat-input__model:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.08);
  border-color: rgba(128, 128, 128, 0.32);
}

.chat-input__model:active:not(:disabled) {
  transform: scale(0.97);
}

.chat-input__model:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-input__model--active {
  color: rgba(128, 128, 128, 0.9);
  border-color: rgba(128, 128, 128, 0.35);
  background: rgba(128, 128, 128, 0.08);
}

.chat-input__model--active:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.12);
  border-color: rgba(128, 128, 128, 0.45);
}

.chat-input__model--collapsed {
  width: 34px;
  max-width: 34px;
  padding: 0;
  justify-content: center;
}

.chat-input__model--collapsed .chat-input__model-label,
.chat-input__model--collapsed .chat-input__model-chevron {
  max-width: 0;
  opacity: 0;
  margin-left: -5px;
}

.chat-input__model--collapsed:hover:not(:disabled) {
  width: auto;
  max-width: 180px;
  padding: 0 8px 0 10px;
  justify-content: flex-start;
}

.chat-input__model--collapsed:hover:not(:disabled) .chat-input__model-label {
  max-width: 110px;
  opacity: 1;
  margin-left: 0;
}

.chat-input__model--collapsed:hover:not(:disabled) .chat-input__model-chevron {
  max-width: 14px;
  opacity: 1;
  margin-left: 0;
}

.chat-input__model-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 110px;
  line-height: 1.3;
  opacity: 1;
  transition:
    max-width 0.5s ease,
    opacity 0.35s ease,
    margin 0.35s ease;
}

.chat-input__model-chevron {
  transition:
    transform 0.2s ease,
    max-width 0.5s ease,
    opacity 0.35s ease,
    margin 0.35s ease;
}

.chat-input__model-chevron--open {
  transform: rotate(180deg);
}

.chat-input__model-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.chat-input__model-dropdown {
  position: absolute;
  bottom: calc(100% + 8px);
  left: 0;
  z-index: 6000;
  min-width: 240px;
  max-width: 340px;
  max-height: 320px;
  display: flex;
  flex-direction: column;
  padding: 6px 0;
  border-radius: 12px;
  background: rgba(30, 30, 30, 0.5);
  border: 1px solid rgba(128, 128, 128, 0.18);
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
  overflow: hidden;
}

.chat-input__model-search {
  display: flex;
  align-items: center;
  margin: 4px 12px 6px;
  padding: 6px 10px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.12);
  border: 1px solid rgba(128, 128, 128, 0.18);
}

.chat-input__model-search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--chat-text);
  font-size: 0.85em;
  line-height: 1.3;
}

.chat-input__model-search-input::placeholder {
  color: rgba(128, 128, 128, 0.7);
}

.chat-input__model-fade-enter-active,
.chat-input__model-fade-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}

.chat-input__model-fade-enter-from,
.chat-input__model-fade-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.chat-input__model-header {
  font-size: 0.75em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  padding: 10px 16px 6px;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
}

.chat-input__model-separator {
  height: 1px;
  margin: 4px 12px;
  background: rgba(128, 128, 128, 0.18);
}

.chat-input__model-list {
  list-style: none;
  margin: 0;
  padding: 0;
  min-width: 220px;
  max-width: 320px;
  max-height: 220px;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.chat-input__model-item {
  display: flex;
  align-items: center;
  min-height: 40px;
  padding: 6px 12px;
  margin: 2px 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.12s ease;
}

.chat-input__model-item:hover {
  background: rgba(128, 128, 128, 0.12);
}

.chat-input__model-item--active {
  background: rgba(25, 118, 210, 0.1);
}

.chat-input__model-item-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  padding-right: 8px;
}

.chat-input__model-name {
  font-size: 0.86em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chat-input__model-status {
  display: flex;
  align-items: center;
  min-height: 40px;
  padding: 6px 12px;
  margin: 2px 8px;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
  font-size: 0.85em;
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
