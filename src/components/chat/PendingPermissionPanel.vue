<template>
  <div class="permission-panel">
    <div class="permission-panel__header">
      <span class="permission-panel__dot" />
      <span class="permission-panel__label">{{ $t("chat.permissionRequired") }}</span>
      <span class="permission-panel__tool">{{ request.toolName }}</span>
    </div>
    <div v-if="request.reason" class="permission-panel__reason">
      {{ request.reason }}
    </div>

    <div class="permission-panel__actions">
      <button
        v-for="option in request.options"
        :key="option.optionId"
        type="button"
        class="permission-panel__btn"
        :class="isReject(option) ? 'permission-panel__btn--reject' : 'permission-panel__btn--allow'"
        @click="onChoose(option.optionId)"
      >
        <q-icon :name="isReject(option) ? 'close' : 'check'" size="14px" />
        {{ optionLabel(option) }}
      </button>
    </div>
  </div>
</template>

<script setup>
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";

const props = defineProps({
  request: { type: Object, required: true },
});

const { t } = useI18n();
const zeroStore = useZeroStore();

function isReject(option) {
  const kind = option.kind || "";
  return kind.startsWith("reject") || kind.startsWith("deny");
}

// zero's own `option.name` is always in English, and several distinct
// options can share the same broad `kind` (verified live: a single bash
// permission request offered three separate `allow_always` options with
// different meanings) - so neither `name` nor `kind` alone gives a correct,
// localized, non-duplicate label. `optionId` is the stable per-option
// identifier zero actually varies, so the known ones are mapped to real
// i18n strings here; anything new falls back to a generic label for its
// `kind` bucket, and only to zero's raw `name` if even that is unrecognized.
const OPTION_ID_LABEL_KEYS = {
  allow: "chat.permissionOptionAllow",
  allow_for_session: "chat.permissionOptionAllowSession",
  allow_prefix_for_session: "chat.permissionOptionAllowPrefixSession",
  always_allow_prefix: "chat.permissionOptionAlwaysAllowPrefix",
  deny: "chat.permissionOptionDeny",
};

const KIND_LABEL_KEYS = {
  allow_once: "chat.permissionOptionAllow",
  allow_always: "chat.permissionOptionAllowAlways",
  reject_once: "chat.permissionOptionDeny",
  reject_always: "chat.permissionOptionDeny",
};

function optionLabel(option) {
  const key = OPTION_ID_LABEL_KEYS[option.optionId] || KIND_LABEL_KEYS[option.kind];
  return key ? t(key) : option.name || option.optionId;
}

function onChoose(optionId) {
  zeroStore.respondToPermission(props.request.requestId, optionId);
}
</script>

<style scoped>
.permission-panel {
  display: flex;
  flex-direction: column;
  border-radius: 18px;
  background: rgba(128, 128, 128, 0.09);
  border: 1px solid rgba(242, 192, 55, 0.35);
  margin-bottom: 8px;
  overflow: hidden;
  transition:
    border-color 0.15s ease,
    background 0.15s ease;
}

body.body--dark .permission-panel {
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(242, 192, 55, 0.3);
}

.permission-panel__header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px 2px;
  font-size: 0.8em;
  color: #f2c037;
}

.permission-panel__dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: #f2c037;
  animation: permission-panel-pulse 1.5s ease-in-out infinite;
}

.permission-panel__label {
  font-weight: 600;
  white-space: nowrap;
}

.permission-panel__tool {
  color: var(--chat-text);
  opacity: 0.8;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.permission-panel__reason {
  padding: 2px 16px 4px;
  font-size: 0.8em;
  color: var(--chat-text-muted);
}

.permission-panel__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 8px 16px 12px;
}

.permission-panel__btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  border-radius: 14px;
  padding: 6px 12px;
  font-family: inherit;
  font-size: 0.82em;
  line-height: 1.2;
  cursor: pointer;
  transition:
    background 0.15s ease,
    transform 0.1s ease;
}

.permission-panel__btn:hover {
  transform: scale(1.04);
}

.permission-panel__btn:active {
  transform: scale(0.96);
}

.permission-panel__btn--allow {
  background: rgba(33, 186, 69, 0.14);
  color: #21ba45;
}

.permission-panel__btn--allow:hover {
  background: rgba(33, 186, 69, 0.24);
}

.permission-panel__btn--reject {
  background: rgba(244, 67, 54, 0.14);
  color: #f44336;
}

.permission-panel__btn--reject:hover {
  background: rgba(244, 67, 54, 0.24);
}

@keyframes permission-panel-pulse {
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
</style>
