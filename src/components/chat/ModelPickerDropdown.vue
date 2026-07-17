<template>
  <div class="model-picker">
    <button
      type="button"
      class="model-picker__button"
      :class="{
        'model-picker__button--active': modelValue,
        'model-picker__button--collapsed': collapsed,
      }"
      :disabled="disabled"
      @click="toggleOpen"
    >
      <q-icon name="memory" size="14px" />
      <span class="model-picker__label">{{ modelValue || placeholderLabel }}</span>
      <q-icon
        name="expand_more"
        size="14px"
        class="model-picker__chevron"
        :class="{ 'model-picker__chevron--open': open }"
      />
    </button>
    <transition name="model-picker__fade">
      <div v-if="open" v-click-outside="close" class="model-picker__dropdown">
        <div class="model-picker__header row items-center justify-between">
          <span>{{ headerTitle }}</span>
          <q-icon name="memory" size="14px" color="grey-6" />
        </div>
        <div class="model-picker__separator" />
        <div class="model-picker__search">
          <q-icon name="search" size="14px" color="grey-6" class="q-mr-sm" />
          <input
            v-model="search"
            type="text"
            :placeholder="t('chat.searchModel')"
            class="model-picker__search-input"
            @click.stop
          />
        </div>
        <div v-if="showRecents && recentModels.length && !search" class="model-picker__section">
          <div class="model-picker__section-title">{{ t("chat.recentModels") }}</div>
          <ul class="model-picker__list model-picker__list--recent">
            <li
              v-for="m in recentModels"
              :key="`recent-${m}`"
              :class="['model-picker__item', { 'model-picker__item--active': m === modelValue }]"
              @click="select(m)"
            >
              <span class="model-picker__item-avatar">
                <q-icon v-if="m === modelValue" name="check_circle" size="18px" color="primary" />
                <q-icon v-else name="history" size="18px" color="grey-6" />
              </span>
              <span class="model-picker__name">{{ m }}</span>
            </li>
          </ul>
          <div class="model-picker__separator" />
        </div>
        <ul class="model-picker__list">
          <li
            v-if="allowClear"
            :class="['model-picker__item', { 'model-picker__item--active': !modelValue }]"
            @click="select(null)"
          >
            <span class="model-picker__item-avatar">
              <q-icon
                :name="!modelValue ? 'check_circle' : 'radio_button_unchecked'"
                size="18px"
                :color="!modelValue ? 'primary' : 'grey-6'"
              />
            </span>
            <span class="model-picker__name">{{ placeholderLabel }}</span>
          </li>
          <li
            v-for="m in filteredModels"
            :key="m"
            :class="['model-picker__item', { 'model-picker__item--active': m === modelValue }]"
            @click="select(m)"
          >
            <span class="model-picker__item-avatar">
              <q-icon v-if="m === modelValue" name="check_circle" size="18px" color="primary" />
              <q-icon v-else name="radio_button_unchecked" size="18px" color="grey-6" />
            </span>
            <span class="model-picker__name">{{ m }}</span>
          </li>
          <li v-if="zeroStore.isLoadingModels" class="model-picker__status">
            <span class="model-picker__item-avatar">
              <q-spinner-dots size="18px" color="primary" />
            </span>
            <span>{{ t("chat.loadingModels") }}</span>
          </li>
          <li v-else-if="filteredModels.length === 0" class="model-picker__status">
            <span class="model-picker__item-avatar">
              <q-icon name="search_off" size="18px" color="grey-6" />
            </span>
            <span>{{ t("chat.noModelsMatch") }}</span>
          </li>
        </ul>
      </div>
    </transition>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";
import { vClickOutside } from "@/utils/click-outside";

const MAX_RECENT_MODELS = 3;

const props = defineProps({
  modelValue: { type: String, default: null },
  // Button text (and, when allowClear, the "use default" list entry) shown
  // when nothing is selected.
  placeholderLabel: { type: String, required: true },
  // Dropdown header text. Falls back to placeholderLabel's chat.switchModel
  // sibling when omitted.
  title: { type: String, default: "" },
  disabled: { type: Boolean, default: false },
  collapsed: { type: Boolean, default: false },
  // Recent-picks section, persisted per caller under this localStorage key.
  // Omit both to skip the section entirely (e.g. a secondary picker that
  // doesn't warrant its own recency tracking).
  showRecents: { type: Boolean, default: false },
  recentsStorageKey: { type: String, default: "" },
  // Shows a "use placeholderLabel" entry that selects null - for pickers
  // where "unset" is a meaningful, distinct choice (e.g. the advisor
  // model, where null means "same as the executor"), not just an empty
  // state to fill in.
  allowClear: { type: Boolean, default: false },
});

const emit = defineEmits(["update:modelValue"]);

const { t } = useI18n();
const zeroStore = useZeroStore();

const open = ref(false);
const search = ref("");
const headerTitle = computed(() => props.title || t("chat.switchModel"));

onMounted(() => {
  zeroStore.loadAvailableModels();
});

const recentModels = computed(() => {
  if (!props.showRecents || !props.recentsStorageKey) return [];
  const active = props.modelValue;
  const recent = JSON.parse(localStorage.getItem(props.recentsStorageKey) || "[]").filter(
    (m) => typeof m === "string" && zeroStore.availableModels.includes(m) && m !== active,
  );
  if (active && zeroStore.availableModels.includes(active)) {
    return [active, ...recent.filter((m) => m !== active)].slice(0, MAX_RECENT_MODELS);
  }
  return recent.slice(0, MAX_RECENT_MODELS);
});

const filteredModels = computed(() => {
  const query = search.value.trim().toLowerCase();
  const active = props.modelValue;
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

function rememberRecent(model) {
  if (!props.showRecents || !props.recentsStorageKey || !model) return;
  const recent = JSON.parse(localStorage.getItem(props.recentsStorageKey) || "[]").filter(
    (m) => typeof m === "string" && m !== model,
  );
  recent.unshift(model);
  localStorage.setItem(props.recentsStorageKey, JSON.stringify(recent.slice(0, MAX_RECENT_MODELS)));
}

async function toggleOpen() {
  if (open.value) {
    close();
    return;
  }
  open.value = true;
  await zeroStore.loadAvailableModels();
}

function close() {
  open.value = false;
  search.value = "";
}

function select(model) {
  rememberRecent(model);
  emit("update:modelValue", model);
  close();
}
</script>

<style scoped>
.model-picker {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.model-picker__button {
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

.model-picker__button:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.08);
  border-color: rgba(128, 128, 128, 0.32);
}

.model-picker__button:active:not(:disabled) {
  transform: scale(0.97);
}

.model-picker__button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.model-picker__button--active {
  color: rgba(128, 128, 128, 0.9);
  border-color: rgba(128, 128, 128, 0.35);
  background: rgba(128, 128, 128, 0.08);
}

.model-picker__button--active:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.12);
  border-color: rgba(128, 128, 128, 0.45);
}

.model-picker__button--collapsed {
  width: 34px;
  max-width: 34px;
  padding: 0;
  justify-content: center;
}

.model-picker__button--collapsed .model-picker__label,
.model-picker__button--collapsed .model-picker__chevron {
  max-width: 0;
  opacity: 0;
  margin-left: -5px;
}

.model-picker__label {
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

.model-picker__chevron {
  transition:
    transform 0.2s ease,
    max-width 0.5s ease,
    opacity 0.35s ease,
    margin 0.35s ease;
}

.model-picker__chevron--open {
  transform: rotate(180deg);
}

.model-picker__dropdown {
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

.model-picker__search {
  display: flex;
  align-items: center;
  margin: 4px 12px 6px;
  padding: 6px 10px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.12);
  border: 1px solid rgba(128, 128, 128, 0.18);
}

.model-picker__search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--chat-text);
  font-size: 0.85em;
  line-height: 1.3;
}

.model-picker__search-input::placeholder {
  color: rgba(128, 128, 128, 0.7);
}

.model-picker__fade-enter-active,
.model-picker__fade-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}

.model-picker__fade-enter-from,
.model-picker__fade-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.model-picker__header {
  font-size: 0.75em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  padding: 10px 16px 6px;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
}

.model-picker__separator {
  height: 1px;
  margin: 4px 12px;
  background: rgba(128, 128, 128, 0.18);
}

.model-picker__section-title {
  font-size: 0.72em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  padding: 4px 16px;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.7));
}

.model-picker__list {
  list-style: none;
  margin: 0;
  padding: 0;
  min-width: 220px;
  max-width: 320px;
  max-height: 220px;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.model-picker__item {
  display: flex;
  align-items: center;
  min-height: 40px;
  padding: 6px 12px;
  margin: 2px 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.12s ease;
}

.model-picker__item:hover {
  background: rgba(128, 128, 128, 0.12);
}

.model-picker__item--active {
  background: rgba(25, 118, 210, 0.1);
}

.model-picker__item-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  padding-right: 8px;
}

.model-picker__name {
  font-size: 0.86em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.model-picker__status {
  display: flex;
  align-items: center;
  min-height: 40px;
  padding: 6px 12px;
  margin: 2px 8px;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
  font-size: 0.85em;
}
</style>
