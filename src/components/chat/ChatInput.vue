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
      <div class="chat-input__stack">
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
        <div class="chat-input__toolbar">
          <div v-if="activeModelReasoningEfforts.length" class="chat-input__effort-wrap">
            <button
              type="button"
              class="chat-input__effort"
              :class="{
                'chat-input__effort--active': sessionStore.reasoningEffort !== '',
                'chat-input__effort--collapsed': isNarrowViewport,
              }"
              :disabled="disabled"
              @click="toggleEffortMenu"
            >
              <q-icon name="psychology" size="14px" />
              <span class="chat-input__effort-label">{{ effortLabel }}</span>
              <q-tooltip v-if="!effortMenuOpen">{{ t("chat.effortTooltip") }}</q-tooltip>
            </button>
            <transition name="chat-input__model-fade">
              <div
                v-if="effortMenuOpen"
                v-click-outside="closeEffortMenu"
                class="chat-input__effort-dropdown"
              >
                <div class="chat-input__model-header">{{ t("chat.effortLabel") }}</div>
                <div class="chat-input__model-separator" />
                <button
                  v-for="option in effortOptions"
                  :key="option.value"
                  type="button"
                  class="chat-input__effort-item"
                  :class="{
                    'chat-input__effort-item--active': currentEffort === option.value,
                  }"
                  @click="selectReasoningEffort(option.value)"
                >
                  <q-icon
                    :name="
                      currentEffort === option.value ? 'check_circle' : 'radio_button_unchecked'
                    "
                    size="16px"
                    :color="currentEffort === option.value ? 'primary' : 'grey-6'"
                  />
                  <span class="chat-input__effort-item-label">{{ option.label }}</span>
                </button>
              </div>
            </transition>
          </div>
          <div class="chat-input__advisor-wrap">
            <div
              class="chat-input__advisor-pill"
              :class="{
                'chat-input__advisor-pill--active': sessionStore.advisorEnabled,
                'chat-input__advisor-pill--collapsed': isNarrowViewport,
              }"
            >
              <button
                type="button"
                class="chat-input__advisor-toggle"
                :disabled="disabled"
                @click="toggleAdvisorMode"
              >
                <q-icon name="auto_awesome" size="14px" />
                <span class="chat-input__advisor-toggle-label">{{ advisorModeLabel }}</span>
                <q-tooltip>{{ t("chat.advisorTooltip") }}</q-tooltip>
              </button>
              <template v-if="sessionStore.advisorEnabled">
                <div class="chat-input__advisor-divider" />
                <button
                  type="button"
                  class="chat-input__advisor-gear"
                  :disabled="disabled"
                  @click="toggleAdvisorSettings"
                >
                  <q-icon name="settings" size="13px" />
                  <q-tooltip>{{ t("chat.advisorSettings") }}</q-tooltip>
                </button>
              </template>
            </div>
            <transition name="chat-input__model-fade">
              <div
                v-if="advisorSettingsOpen"
                v-click-outside="closeAdvisorSettings"
                class="chat-input__advisor-settings-popup"
              >
                <div class="chat-input__model-header">{{ t("chat.advisorSettingsTitle") }}</div>
                <div class="chat-input__model-separator" />
                <div class="chat-input__advisor-settings-row">
                  <span class="chat-input__advisor-settings-label">{{
                    t("chat.executorModelLabel")
                  }}</span>
                  <ModelPickerDropdown
                    :model-value="effectiveActiveModel"
                    :placeholder-label="t('chat.modelLabel')"
                    :disabled="disabled"
                    show-recents
                    recents-storage-key="zero-recent-models"
                    @update:model-value="selectModel"
                  />
                </div>
                <div class="chat-input__advisor-settings-row">
                  <span class="chat-input__advisor-settings-label">{{
                    t("chat.advisorModelLabel")
                  }}</span>
                  <ModelPickerDropdown
                    :model-value="sessionStore.advisorModel"
                    :placeholder-label="t('chat.advisorModelDefault')"
                    :title="t('chat.advisorModelLabel')"
                    :disabled="disabled"
                    allow-clear
                    @update:model-value="selectAdvisorModel"
                  />
                </div>
                <div class="chat-input__advisor-settings-row">
                  <span class="chat-input__advisor-settings-label">{{
                    t("chat.advisorTriggerModeLabel")
                  }}</span>
                  <q-btn-toggle
                    :model-value="sessionStore.advisorMode"
                    dense
                    unelevated
                    no-caps
                    toggle-color="primary"
                    size="sm"
                    :disabled="disabled"
                    :options="[
                      { label: t('chat.advisorModeMax'), value: 'max' },
                      { label: t('chat.advisorModeLow'), value: 'low' },
                    ]"
                    @update:model-value="selectAdvisorMode"
                  >
                    <q-tooltip anchor="top middle" self="bottom middle">
                      {{
                        sessionStore.advisorMode === "low"
                          ? t("chat.advisorModeLowTooltip")
                          : t("chat.advisorModeMaxTooltip")
                      }}
                    </q-tooltip>
                  </q-btn-toggle>
                </div>
              </div>
            </transition>
          </div>
          <div class="chat-input__mode-wrap">
            <button
              type="button"
              class="chat-input__mode"
              :class="{
                'chat-input__mode--active': sessionStore.sessionMode !== 'auto',
                'chat-input__mode--collapsed': isNarrowViewport,
              }"
              :disabled="disabled"
              @click="toggleModeMenu"
            >
              <q-icon :name="sessionModeIcon" size="14px" />
              <span class="chat-input__mode-label">{{ sessionModeLabel }}</span>
              <q-tooltip v-if="!modeMenuOpen">{{ sessionModeTooltip }}</q-tooltip>
            </button>
            <transition name="chat-input__model-fade">
              <div
                v-if="modeMenuOpen"
                v-click-outside="closeModeMenu"
                class="chat-input__mode-dropdown"
              >
                <div class="chat-input__model-header">{{ t("chat.modeMenuTitle") }}</div>
                <div class="chat-input__model-separator" />
                <button
                  v-for="option in sessionModeOptions"
                  :key="option.value"
                  type="button"
                  class="chat-input__mode-item"
                  :class="{
                    'chat-input__mode-item--active': sessionStore.sessionMode === option.value,
                  }"
                  @click="selectSessionMode(option.value)"
                >
                  <q-icon
                    :name="sessionStore.sessionMode === option.value ? 'check_circle' : option.icon"
                    size="16px"
                    :color="sessionStore.sessionMode === option.value ? 'primary' : 'grey-6'"
                  />
                  <span class="chat-input__mode-item-text">
                    <span class="chat-input__mode-item-label">{{ option.label }}</span>
                    <span class="chat-input__mode-item-desc">{{ option.description }}</span>
                  </span>
                </button>
              </div>
            </transition>
          </div>
          <ModelPickerDropdown
            v-if="!sessionStore.advisorEnabled"
            :model-value="effectiveActiveModel"
            :placeholder-label="t('chat.modelLabel')"
            :disabled="disabled"
            :collapsed="isNarrowViewport"
            show-recents
            recents-storage-key="zero-recent-models"
            @update:model-value="selectModel"
          />
        </div>
      </div>
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
import { ref, computed, nextTick, watch, onMounted, inject } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { planIcon, planColor } from "@/utils/plan";
import { readFileAttachment } from "@/services/zero";
import { base64ToObjectUrl, base64ToDataUri } from "@/utils/image";
import { isImageMimeType, isTextMimeType, getFileIcon } from "@/utils/file";
import { useZeroStore } from "@/stores/zero-store";
import { vClickOutside } from "@/utils/click-outside";
import ModelPickerDropdown from "@/components/chat/ModelPickerDropdown.vue";

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
// Lives on the per-session store (not a local ref) so the terminal panel's
// "cite to chat" action can set it on whichever panel is focused - see
// zero-session-store.js's pendingAttachment.
const attachedFile = computed({
  get: () => sessionStore.pendingAttachment,
  set: (value) => {
    sessionStore.pendingAttachment = value;
  },
});
const pickingFile = ref(false);
const advisorSettingsOpen = ref(false);
const modeMenuOpen = ref(false);
const effortMenuOpen = ref(false);

const paneWidth = inject("paneWidth", ref(9999));
// Same threshold as ChatView.vue's PANE_NARROW_THRESHOLD - kept in sync so
// the pills collapse to icon-only right as the pane itself switches into its
// narrow layout, instead of two independently-tuned breakpoints drifting
// apart. Deliberately higher than the pills' actual minimum combined width
// so collapsing kicks in with room to spare (e.g. right when a second panel
// is opened), rather than only after the row has already started to crowd.
const isNarrowViewport = computed(() => paneWidth.value < 640);

// The session's ACP permission mode - all three are the real
// `session/set_mode`, enforced by the engine itself (see
// zero-session-store.js's setMode): "auto" runs safe tools automatically and
// asks before risky ones, "ask" asks before every tool that changes state,
// "spec-draft" is Plan Mode (read-only exploration ending in a plan for
// review).
const SESSION_MODE_ORDER = ["auto", "ask", "spec-draft"];

const sessionModeMeta = computed(() => ({
  auto: {
    label: t("chat.autoAllow"),
    description: t("chat.autoAllowTooltip"),
    icon: "check_circle",
  },
  ask: {
    label: t("chat.ask"),
    description: t("chat.askTooltip"),
    icon: "help_outline",
  },
  "spec-draft": {
    label: t("chat.planMode"),
    description: t("chat.planModeTooltip"),
    icon: "fact_check",
  },
}));

const sessionModeOptions = computed(() =>
  SESSION_MODE_ORDER.map((value) => ({ value, ...sessionModeMeta.value[value] })),
);

const currentSessionMode = computed(() => sessionStore.sessionMode || "auto");
const sessionModeLabel = computed(() => sessionModeMeta.value[currentSessionMode.value].label);
const sessionModeIcon = computed(() => sessionModeMeta.value[currentSessionMode.value].icon);
const sessionModeTooltip = computed(
  () => sessionModeMeta.value[currentSessionMode.value].description,
);

const advisorModeLabel = computed(() =>
  sessionStore.advisorEnabled ? t("chat.advisorOn") : t("chat.advisorOff"),
);

// The model a session is actually running under is per-session (each
// session's process keeps whatever model it snapshotted when it last
// (re)connected - see zero-session-store.js's switchModel/startSession).
// Falls back to the global store's model only for a panel that hasn't
// connected yet, so it shows the default it would inherit if it did.
const effectiveActiveModel = computed(() => sessionStore.activeModel || zeroStore.activeModel);

// Reasoning-effort tiers the active model supports, sourced from
// zeroStore.modelCapabilities (populated from `zero providers models --json`
// via loadAvailableModels). Empty for a model with no reasoning controls, or
// one not yet resolved by the model registry - the toggle below hides itself
// entirely in that case, mirroring the TUI's /effort picker (which only ever
// offers "auto" for such models).
const activeModelReasoningEfforts = computed(
  () => zeroStore.modelCapabilities[effectiveActiveModel.value]?.reasoningEfforts || [],
);

const effortLabels = {
  minimal: "chat.effortMinimal",
  low: "chat.effortLow",
  medium: "chat.effortMedium",
  high: "chat.effortHigh",
  xhigh: "chat.effortXhigh",
  max: "chat.effortMax",
};

const effortOptions = computed(() => [
  { label: t("chat.effortAuto"), value: "auto" },
  ...activeModelReasoningEfforts.value.map((tier) => ({
    label: effortLabels[tier] ? t(effortLabels[tier]) : tier,
    value: tier,
  })),
]);

const currentEffort = computed(() => sessionStore.reasoningEffort || "auto");
const effortLabel = computed(
  () => effortOptions.value.find((option) => option.value === currentEffort.value)?.label,
);

// Load eagerly on mount instead of only on first click of the model picker,
// so the menu is already populated by the time the user opens it - the
// @show handler on the q-menu stays as a retry path if this fails silently
// (e.g. a transient network hiccup probing the provider's model list).
onMounted(() => {
  zeroStore.loadAvailableModels();
});

// Revokes an image attachment's blob: URL whenever it's actually replaced or
// cleared - covers both this component's own pickFile()/removeAttachedFile()
// (which already revoke explicitly before reassigning) and an attachment set
// from outside (the terminal panel's "cite to chat", which never goes
// through those functions). Deliberately NOT tied to this component's own
// unmount: pendingAttachment lives on the session store, which outlives a
// ChatInput/ChatView instance across a panel-count layout change (1↔2↔3↔4
// panels remounts them) - revoking here on unmount would break the preview
// the moment it remounts.
watch(
  () => sessionStore.pendingAttachment,
  (next, prev) => {
    if (prev?.previewUrl && prev.previewUrl !== next?.previewUrl) {
      URL.revokeObjectURL(prev.previewUrl);
    }
  },
);

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

async function pickFile() {
  // No `filters` - the native dialog shows every file. The backend
  // (attachment_kind_for_file in lib.rs) never rejects by type either: known
  // extensions keep their exact mime type, anything else that looks like
  // text is attached as text, and genuinely binary content (PDFs, archives,
  // ...) is still attached, just sent to the agent as a named reference
  // instead of inlined content (see build_prompt_blocks in bridge.rs).
  const selected = await open({
    multiple: false,
    title: t("chat.attachFileTitle"),
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

function toggleModeMenu() {
  modeMenuOpen.value = !modeMenuOpen.value;
}

function closeModeMenu() {
  modeMenuOpen.value = false;
}

function selectSessionMode(mode) {
  closeModeMenu();
  if (mode === currentSessionMode.value) return;
  sessionStore.setMode(mode);
}

function toggleEffortMenu() {
  effortMenuOpen.value = !effortMenuOpen.value;
}

function closeEffortMenu() {
  effortMenuOpen.value = false;
}

function toggleAdvisorMode() {
  const nextEnabled = !sessionStore.advisorEnabled;
  sessionStore.toggleAdvisor(nextEnabled);
  if (!nextEnabled) {
    // The gear that opens the settings popup only renders while advisor mode
    // is on (see template) - closing here avoids leaving the popup stranded
    // open with no button left to close it via toggleAdvisorSettings.
    advisorSettingsOpen.value = false;
  }
}

function toggleAdvisorSettings() {
  advisorSettingsOpen.value = !advisorSettingsOpen.value;
}

function closeAdvisorSettings() {
  advisorSettingsOpen.value = false;
}

function selectModel(model) {
  sessionStore.switchModel(model);
}

function selectReasoningEffort(effort) {
  closeEffortMenu();
  if (effort === currentEffort.value) return;
  sessionStore.switchEffort(effort === "auto" ? "" : effort);
}

function selectAdvisorModel(model) {
  sessionStore.setAdvisorModel(model);
}

function selectAdvisorMode(mode) {
  sessionStore.setAdvisorMode(mode);
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
  /* overflow: hidden removido — o dropdown absoluto do seletor de modelos
     precisa ultrapassar os limites do .chat-input */
}

.chat-input__row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 6px 6px 16px;
}

/* Second floor: textarea on top, toolbar (effort/advisor/mode/model/send)
   below - the attach button (outside this stack) centers vertically against
   the stack's full height via .chat-input__row's align-items: center. */
.chat-input__stack {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.chat-input__toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
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
  width: 100%;
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

.chat-input__mode-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.chat-input__effort-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
}

/* Same border/radius/height/hover/active treatment as .chat-input__mode - see
   that block's comment for the shared reasoning. */
.chat-input__effort {
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

.chat-input__effort:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.08);
  border-color: rgba(128, 128, 128, 0.32);
}

.chat-input__effort:active:not(:disabled) {
  transform: scale(0.97);
}

.chat-input__effort:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-input__effort--active {
  color: var(--q-primary, #1976d2);
  border-color: rgba(25, 118, 210, 0.35);
  background: rgba(25, 118, 210, 0.06);
}

.chat-input__effort--active:hover:not(:disabled) {
  background: rgba(25, 118, 210, 0.12);
  border-color: rgba(25, 118, 210, 0.45);
}

.chat-input__effort--collapsed {
  width: 34px;
  max-width: 34px;
  padding: 0;
  justify-content: center;
}

.chat-input__effort--collapsed .chat-input__effort-label {
  max-width: 0;
  opacity: 0;
  margin-left: -5px;
}

.chat-input__effort-label {
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

.chat-input__effort-dropdown {
  position: absolute;
  bottom: calc(100% + 8px);
  left: 0;
  z-index: 6000;
  min-width: 180px;
  max-width: 240px;
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

.chat-input__effort-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: calc(100% - 16px);
  min-height: 36px;
  padding: 8px 16px;
  margin: 2px 8px;
  border: none;
  border-radius: 8px;
  background: transparent;
  cursor: pointer;
  text-align: left;
  transition: background 0.12s ease;
}

.chat-input__effort-item:hover {
  background: rgba(128, 128, 128, 0.12);
}

.chat-input__effort-item--active {
  background: rgba(25, 118, 210, 0.1);
}

.chat-input__effort-item-label {
  font-size: 0.86em;
  font-weight: 500;
  color: var(--chat-text);
}

.chat-input__mode-dropdown {
  position: absolute;
  bottom: calc(100% + 8px);
  left: 0;
  z-index: 6000;
  min-width: 260px;
  max-width: 320px;
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

.chat-input__mode-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  width: calc(100% - 16px);
  min-height: 40px;
  padding: 8px 16px;
  margin: 2px 8px;
  border: none;
  border-radius: 8px;
  background: transparent;
  cursor: pointer;
  text-align: left;
  transition: background 0.12s ease;
}

.chat-input__mode-item:hover {
  background: rgba(128, 128, 128, 0.12);
}

.chat-input__mode-item--active {
  background: rgba(25, 118, 210, 0.1);
}

.chat-input__mode-item-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.chat-input__mode-item-label {
  font-size: 0.86em;
  font-weight: 500;
  color: var(--chat-text);
}

.chat-input__mode-item-desc {
  font-size: 0.76em;
  line-height: 1.3;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
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

/* One cohesive pill (border/background/radius live here) housing both the
   toggle and the gear as borderless sub-buttons, rather than two separate
   pill/circle buttons crammed next to each other. */
/* Positioning context only (no border/background/overflow here) - the
   settings popup below is an absolutely-positioned child of THIS element,
   not of .chat-input__advisor-pill, so the pill's own overflow: hidden
   (needed to clip the toggle/divider/gear to its rounded corners) can't
   also clip the popup. */
.chat-input__advisor-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
}

/* Same border/radius/height/hover/active treatment as .chat-input__mode
   (the permission-mode pill right next to it), so the two read as one
   family of controls instead of a one-off design. */
.chat-input__advisor-pill {
  flex-shrink: 0;
  height: 34px;
  display: inline-flex;
  align-items: stretch;
  border-radius: 17px;
  border: 1px solid rgba(128, 128, 128, 0.22);
  background: transparent;
  color: rgba(128, 128, 128, 0.9);
  overflow: hidden;
  transition:
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}

.chat-input__advisor-pill:hover {
  background: rgba(128, 128, 128, 0.08);
  border-color: rgba(128, 128, 128, 0.32);
}

.chat-input__advisor-pill--active {
  color: var(--q-primary, #1976d2);
  border-color: rgba(25, 118, 210, 0.35);
  background: rgba(25, 118, 210, 0.06);
}

.chat-input__advisor-pill--active:hover {
  background: rgba(25, 118, 210, 0.12);
  border-color: rgba(25, 118, 210, 0.45);
}

.chat-input__advisor-toggle {
  flex-shrink: 0;
  height: 100%;
  padding: 0 8px 0 10px;
  border: none;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font-size: 0.82em;
  font-weight: 500;
  transition: padding 0.35s ease;
}

.chat-input__advisor-toggle:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-input__advisor-toggle-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 90px;
  line-height: 1.3;
  opacity: 1;
  transition:
    max-width 0.35s ease,
    opacity 0.25s ease,
    margin 0.25s ease;
}

.chat-input__advisor-pill--collapsed .chat-input__advisor-toggle {
  padding: 0 6px 0 10px;
}

.chat-input__advisor-pill--collapsed .chat-input__advisor-toggle-label {
  max-width: 0;
  opacity: 0;
  margin-left: -5px;
}

.chat-input__advisor-divider {
  flex-shrink: 0;
  width: 1px;
  margin: 7px 0;
  background: rgba(128, 128, 128, 0.22);
}

.chat-input__advisor-pill--active .chat-input__advisor-divider {
  background: rgba(25, 118, 210, 0.25);
}

.chat-input__advisor-gear {
  flex-shrink: 0;
  height: 100%;
  width: 30px;
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: rgba(128, 128, 128, 0.75);
  cursor: pointer;
  transition:
    background 0.15s ease,
    color 0.15s ease;
}

.chat-input__advisor-pill--active .chat-input__advisor-gear {
  color: inherit;
}

.chat-input__advisor-gear:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.14);
  color: rgba(128, 128, 128, 0.95);
}

.chat-input__advisor-gear:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-input__advisor-settings-popup {
  position: absolute;
  bottom: calc(100% + 8px);
  left: 0;
  z-index: 6000;
  min-width: 260px;
  display: flex;
  flex-direction: column;
  padding: 6px 0 12px;
  border-radius: 12px;
  background: rgba(30, 30, 30, 0.5);
  border: 1px solid rgba(128, 128, 128, 0.18);
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
}

.chat-input__advisor-settings-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px 16px;
}

.chat-input__advisor-settings-label {
  font-size: 0.75em;
  font-weight: 600;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
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
