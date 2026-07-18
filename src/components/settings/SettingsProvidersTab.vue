<template>
  <div class="settings-providers">
    <!-- Configured providers -->
    <section class="settings-providers__section">
      <div class="settings-providers__section-header">
        <span class="settings-general__section-title">{{
          $t("settings.providersConfiguredTitle")
        }}</span>
        <button
          type="button"
          class="settings-dialog__header-btn"
          :disabled="zeroStore.isLoadingProviders"
          @click="zeroStore.loadConfiguredProviders({ force: true })"
        >
          <q-icon name="refresh" size="16px" />
          <q-tooltip>{{ $t("mcp.refreshAll") }}</q-tooltip>
        </button>
      </div>

      <div
        v-if="zeroStore.isLoadingProviders && zeroStore.configuredProviders.length === 0"
        class="settings-providers__center"
      >
        <q-spinner-dots size="32px" color="primary" />
      </div>
      <div
        v-else-if="zeroStore.configuredProviders.length === 0"
        class="settings-providers__center"
      >
        <div class="mcp-drawer__hint">{{ $t("settings.providersEmpty") }}</div>
      </div>

      <div v-else class="mcp-drawer__list">
        <div
          v-for="p in zeroStore.configuredProviders"
          :key="p.name"
          class="mcp-card"
          :class="{ 'mcp-card--ok': p.status === 'ok', 'mcp-card--error': p.status === 'error' }"
        >
          <div class="mcp-card__header" style="cursor: default">
            <div class="mcp-card__icon">
              <q-icon name="cloud" size="18px" />
            </div>
            <div class="mcp-card__meta">
              <div class="mcp-card__title">
                {{ p.name }}
                <span v-if="p.active" class="settings-providers__active-badge">{{
                  $t("settings.providerActive")
                }}</span>
              </div>
              <div class="mcp-card__subtitle">
                <span class="mcp-card__type">{{ p.providerKind }}</span>
                <span class="mcp-card__url">{{ truncateUrl(p.baseUrl) }}</span>
                <span>{{ p.model }}</span>
                <q-icon v-if="p.apiKeySet" name="vpn_key" size="12px" />
              </div>
            </div>
            <div class="mcp-card__status">
              <q-icon
                v-if="p._checking"
                name="hourglass_empty"
                size="16px"
                class="mcp-card__status-spin"
              />
              <q-icon
                v-else-if="p.status === 'ok'"
                name="check_circle"
                size="18px"
                class="mcp-card__status-ok"
              />
              <q-icon
                v-else-if="p.status === 'error'"
                name="error"
                size="18px"
                class="mcp-card__status-error"
              />
              <q-icon v-else name="cloud" size="18px" class="mcp-card__status-idle" />
            </div>
          </div>
          <div class="settings-providers__card-actions">
            <button type="button" class="settings-pill" :disabled="p.active" @click="confirmUse(p)">
              {{ $t("settings.providerUse") }}
            </button>
            <button
              type="button"
              class="settings-pill"
              :disabled="p._checking"
              @click="zeroStore.checkProvider(p.name, { connectivity: true })"
            >
              {{ $t("settings.providerTest") }}
            </button>
            <button
              type="button"
              class="settings-pill settings-pill--danger"
              @click="confirmRemove(p)"
            >
              {{ $t("settings.providerRemove") }}
            </button>
          </div>
          <div v-if="p._error" class="settings-providers__card-error">{{ p._error }}</div>
        </div>
      </div>
    </section>

    <!-- Add provider -->
    <section class="settings-providers__section">
      <div class="settings-general__section-title">{{ $t("settings.addProviderTitle") }}</div>

      <div class="settings-field">
        <span class="settings-field__label">{{ $t("settings.selectProviderLabel") }}</span>
        <div class="settings-provider-picker">
          <button
            type="button"
            class="settings-provider-picker__button"
            @click="pickerOpen = !pickerOpen"
          >
            <span>{{
              selectedEntry ? selectedEntry.name : $t("settings.selectProviderLabel")
            }}</span>
            <q-icon name="expand_more" size="16px" />
          </button>
          <div
            v-if="pickerOpen"
            v-click-outside="() => (pickerOpen = false)"
            class="settings-provider-picker__dropdown"
          >
            <template v-for="group in groupedCatalog" :key="group.title">
              <div class="settings-provider-picker__group-title">{{ group.title }}</div>
              <div
                v-for="entry in group.entries"
                :key="entry.id"
                class="settings-provider-picker__item"
                :class="{ 'settings-provider-picker__item--active': entry.id === form.catalogId }"
                @click="selectEntry(entry)"
              >
                {{ entry.name }}
              </div>
            </template>
            <div
              v-if="zeroStore.providerCatalog.length === 0"
              class="settings-provider-picker__empty"
            >
              <q-spinner-dots size="18px" color="primary" />
            </div>
          </div>
        </div>
      </div>

      <template v-if="form.catalogId">
        <label class="settings-field">
          <span class="settings-field__label">{{ $t("settings.nameLabel") }}</span>
          <div
            class="settings-field__input-wrap"
            :class="{ 'settings-field__input-wrap--focused': focusedField === 'name' }"
          >
            <input
              v-model="form.name"
              type="text"
              class="settings-field__input"
              @focus="focusedField = 'name'"
              @blur="focusedField = null"
            />
          </div>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ $t("settings.modelLabel") }}</span>
          <div
            class="settings-field__input-wrap"
            :class="{ 'settings-field__input-wrap--focused': focusedField === 'model' }"
          >
            <input
              v-model="form.model"
              type="text"
              class="settings-field__input"
              @focus="focusedField = 'model'"
              @blur="focusedField = null"
            />
          </div>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ $t("settings.baseUrlLabel") }}</span>
          <div
            class="settings-field__input-wrap"
            :class="{ 'settings-field__input-wrap--focused': focusedField === 'baseUrl' }"
          >
            <input
              v-model="form.baseUrl"
              type="text"
              class="settings-field__input"
              @focus="focusedField = 'baseUrl'"
              @blur="focusedField = null"
            />
          </div>
          <span v-if="isCustom" class="settings-field__hint">{{
            $t("settings.baseUrlCustomHint")
          }}</span>
        </label>

        <label v-if="selectedEntry?.requiresAuth" class="settings-field">
          <span class="settings-field__label">{{ $t("settings.apiKeyLabel") }}</span>
          <div
            class="settings-field__input-wrap"
            :class="{ 'settings-field__input-wrap--focused': focusedField === 'apiKey' }"
          >
            <input
              v-model="form.apiKey"
              type="password"
              autocomplete="off"
              class="settings-field__input"
              @focus="focusedField = 'apiKey'"
              @blur="focusedField = null"
            />
          </div>
          <span class="settings-field__hint">{{ $t("settings.apiKeyHint") }}</span>
        </label>

        <button
          type="button"
          class="settings-providers__advanced-toggle"
          @click="showAdvanced = !showAdvanced"
        >
          <q-icon :name="showAdvanced ? 'expand_less' : 'expand_more'" size="15px" />
          {{ $t("settings.advancedToggle") }}
        </button>

        <template v-if="showAdvanced">
          <label class="settings-field">
            <span class="settings-field__label">{{ $t("settings.apiKeyEnvLabel") }}</span>
            <div
              class="settings-field__input-wrap"
              :class="{ 'settings-field__input-wrap--focused': focusedField === 'apiKeyEnv' }"
            >
              <input
                v-model="form.apiKeyEnv"
                type="text"
                class="settings-field__input"
                @focus="focusedField = 'apiKeyEnv'"
                @blur="focusedField = null"
              />
            </div>
          </label>

          <label class="settings-field">
            <span class="settings-field__label">{{ $t("settings.authHeaderLabel") }}</span>
            <div
              class="settings-field__input-wrap"
              :class="{ 'settings-field__input-wrap--focused': focusedField === 'authHeader' }"
            >
              <input
                v-model="form.authHeader"
                type="text"
                class="settings-field__input"
                @focus="focusedField = 'authHeader'"
                @blur="focusedField = null"
              />
            </div>
          </label>

          <label class="settings-field">
            <span class="settings-field__label">{{ $t("settings.authSchemeLabel") }}</span>
            <div
              class="settings-field__input-wrap"
              :class="{ 'settings-field__input-wrap--focused': focusedField === 'authScheme' }"
            >
              <input
                v-model="form.authScheme"
                type="text"
                class="settings-field__input"
                @focus="focusedField = 'authScheme'"
                @blur="focusedField = null"
              />
            </div>
          </label>

          <div class="settings-field">
            <span class="settings-field__label">{{ $t("settings.headersLabel") }}</span>
            <div
              v-for="(header, i) in form.headers"
              :key="i"
              class="settings-providers__header-row"
            >
              <input
                v-model="header.key"
                type="text"
                class="settings-field__input settings-field__input--sm"
                placeholder="Header"
              />
              <input
                v-model="header.value"
                type="text"
                class="settings-field__input settings-field__input--sm"
                placeholder="Value"
              />
              <button
                type="button"
                class="settings-dialog__header-btn"
                @click="form.headers.splice(i, 1)"
              >
                <q-icon name="close" size="14px" />
              </button>
            </div>
            <button
              type="button"
              class="settings-pill"
              @click="form.headers.push({ key: '', value: '' })"
            >
              <q-icon name="add" size="14px" />
              {{ $t("settings.headersLabel") }}
            </button>
          </div>
        </template>

        <label class="settings-providers__set-active-row">
          <q-checkbox v-model="form.setActive" color="primary" dense />
          <span>{{ $t("settings.setActiveCheckbox") }}</span>
        </label>
        <div class="settings-providers__warning">{{ $t("settings.setActiveWarning") }}</div>

        <button
          type="button"
          class="settings-pill settings-pill--primary settings-providers__submit"
          :disabled="!canSubmit || submitting"
          @click="onSubmit"
        >
          <q-spinner-dots v-if="submitting" size="16px" />
          <span v-else>{{ $t("settings.addProviderSubmit") }}</span>
        </button>
      </template>
    </section>
  </div>
</template>

<script setup>
import { reactive, ref, computed } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";
import { vClickOutside } from "@/utils/click-outside";

const $q = useQuasar();
const { t } = useI18n();
const zeroStore = useZeroStore();

const pickerOpen = ref(false);
const showAdvanced = ref(false);
const focusedField = ref(null);
const submitting = ref(false);

const form = reactive({
  catalogId: "",
  name: "",
  model: "",
  baseUrl: "",
  apiKey: "",
  apiKeyEnv: "",
  authHeader: "",
  authScheme: "",
  headers: [],
  setActive: false,
});

const selectedEntry = computed(() =>
  zeroStore.providerCatalog.find((entry) => entry.id === form.catalogId),
);

const isCustom = computed(() => form.catalogId.startsWith("custom-"));

const groupedCatalog = computed(() => {
  const cloud = [];
  const local = [];
  const custom = [];
  for (const entry of zeroStore.providerCatalog) {
    if (entry.id.startsWith("custom-")) custom.push(entry);
    else if (entry.local) local.push(entry);
    else cloud.push(entry);
  }
  return [
    { title: t("settings.groupCloud"), entries: cloud },
    { title: t("settings.groupLocal"), entries: local },
    { title: t("settings.groupCustom"), entries: custom },
  ].filter((group) => group.entries.length > 0);
});

function selectEntry(entry) {
  form.catalogId = entry.id;
  form.name = entry.id;
  form.model = entry.defaultModel || "";
  form.baseUrl = entry.defaultBaseUrl || "";
  form.apiKey = "";
  pickerOpen.value = false;
}

// Custom entries ship a placeholder base URL (https://example.invalid/...)
// that must be replaced before the profile makes sense - required entries
// (name always, base URL only for custom-*) gate the submit button.
const canSubmit = computed(() => {
  if (!form.catalogId || !form.name.trim()) return false;
  if (isCustom.value && (!form.baseUrl.trim() || form.baseUrl.includes("example.invalid"))) {
    return false;
  }
  return true;
});

function resetForm() {
  form.catalogId = "";
  form.name = "";
  form.model = "";
  form.baseUrl = "";
  form.apiKey = "";
  form.apiKeyEnv = "";
  form.authHeader = "";
  form.authScheme = "";
  form.headers = [];
  form.setActive = false;
  showAdvanced.value = false;
}

async function onSubmit() {
  submitting.value = true;
  try {
    await zeroStore.addProvider({
      catalogId: form.catalogId,
      name: form.name.trim(),
      model: form.model.trim() || undefined,
      baseUrl: form.baseUrl.trim() || undefined,
      apiKeyEnv: form.apiKeyEnv.trim() || undefined,
      authHeader: form.authHeader.trim() || undefined,
      authScheme: form.authScheme.trim() || undefined,
      authHeaderValue: form.apiKey.trim() || undefined,
      headers: form.headers.filter((h) => h.key.trim()).map((h) => [h.key.trim(), h.value.trim()]),
      setActive: form.setActive,
    });
    $q.notify({ type: "positive", message: t("settings.addProviderSuccess") });
    resetForm();
  } catch (err) {
    $q.notify({
      type: "negative",
      message: t("settings.addProviderError", { error: String(err) }),
    });
  } finally {
    submitting.value = false;
  }
}

function confirmUse(provider) {
  $q.dialog({
    title: t("settings.providerUseConfirmTitle"),
    message: t("settings.providerUseConfirmMessage", { name: provider.name }),
    cancel: true,
    persistent: true,
  }).onOk(async () => {
    try {
      await zeroStore.useProvider(provider.name);
    } catch (err) {
      $q.notify({ type: "negative", message: String(err) });
    }
  });
}

function confirmRemove(provider) {
  $q.dialog({
    title: t("settings.providerRemoveConfirmTitle"),
    message: provider.active
      ? t("settings.providerRemoveActiveWarning", { name: provider.name })
      : t("settings.providerRemoveConfirmMessage", { name: provider.name }),
    cancel: true,
    persistent: true,
  }).onOk(async () => {
    try {
      await zeroStore.removeProvider(provider.name);
    } catch (err) {
      $q.notify({ type: "negative", message: String(err) });
    }
  });
}

function truncateUrl(url) {
  if (!url) return "";
  try {
    return new URL(url).hostname;
  } catch {
    return url.length > 28 ? url.slice(0, 28) + "…" : url;
  }
}
</script>

<style scoped>
.settings-providers {
  display: flex;
  flex-direction: column;
  gap: 26px;
}

/* Duplicated verbatim from SettingsGeneralTab.vue/SettingsDialog.vue -
   `<style scoped>` doesn't cross component files, same reasoning as
   McpDrawer.vue's duplicated .tool-diff-block rules. */
.settings-general__section-title {
  font-size: 0.75em;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: var(--chat-text-muted);
}

.settings-dialog__header-btn {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: rgba(128, 128, 128, 0.85);
  cursor: pointer;
  transition:
    background 0.15s ease,
    color 0.15s ease,
    transform 0.1s ease;
}

.settings-dialog__header-btn:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.14);
  color: var(--chat-text);
  transform: scale(1.06);
}

.settings-dialog__header-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.settings-providers__section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.settings-providers__center {
  display: flex;
  justify-content: center;
  padding: 24px;
}

/* Duplicated verbatim from McpDrawer.vue's .mcp-drawer__list/.mcp-card* -
   `<style scoped>` doesn't cross component files, same reasoning as that
   file's own duplicated .tool-diff-block rules. */
.mcp-drawer__list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.mcp-drawer__hint {
  font-size: 0.9em;
  font-weight: 500;
  color: var(--chat-text);
}

.mcp-card {
  border-radius: 14px;
  background: rgba(128, 128, 128, 0.06);
  border: 1px solid rgba(128, 128, 128, 0.12);
  transition:
    border-color 0.15s ease,
    background 0.15s ease,
    box-shadow 0.15s ease;
}

.mcp-card--ok {
  border-color: rgba(33, 186, 69, 0.25);
}

.mcp-card--error {
  border-color: rgba(244, 67, 54, 0.22);
}

.mcp-card__header {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  border: none;
  background: transparent;
  text-align: left;
  color: inherit;
  border-radius: 14px;
}

.mcp-card__icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(128, 128, 128, 0.12);
  color: rgba(128, 128, 128, 0.85);
}

.mcp-card--ok .mcp-card__icon {
  background: rgba(33, 186, 69, 0.12);
  color: #21ba45;
}

.mcp-card--error .mcp-card__icon {
  background: rgba(244, 67, 54, 0.12);
  color: #f44336;
}

.mcp-card__meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.mcp-card__title {
  font-size: 0.9em;
  font-weight: 600;
  color: var(--chat-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mcp-card__subtitle {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.78em;
  color: var(--chat-text-muted);
  min-width: 0;
}

.mcp-card__type {
  flex-shrink: 0;
  padding: 2px 7px;
  border-radius: 6px;
  font-size: 0.85em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  background: rgba(128, 128, 128, 0.12);
  color: rgba(128, 128, 128, 0.85);
}

.mcp-card__url {
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mcp-card__status {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  color: rgba(128, 128, 128, 0.65);
}

.mcp-card__status-ok {
  color: #21ba45;
}

.mcp-card__status-error {
  color: #f44336;
}

.mcp-card__status-idle {
  color: rgba(128, 128, 128, 0.55);
}

.mcp-card__status-spin {
  animation: settings-providers-spin 1.2s linear infinite;
  color: rgba(128, 128, 128, 0.65);
}

@keyframes settings-providers-spin {
  to {
    transform: rotate(360deg);
  }
}

.settings-providers__card-actions {
  display: flex;
  gap: 6px;
  padding: 0 14px 12px;
}

.settings-providers__card-error {
  padding: 0 14px 10px;
  font-size: 0.78em;
  color: #f44336;
}

.settings-providers__active-badge {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 7px;
  border-radius: 999px;
  font-size: 0.72em;
  font-weight: 600;
  background: rgba(33, 186, 69, 0.14);
  color: #21ba45;
}

.settings-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 14px;
}

.settings-field__label {
  font-size: 0.8em;
  color: var(--chat-text-muted);
}

.settings-field__hint {
  font-size: 0.75em;
  color: var(--chat-text-muted);
}

.settings-field__input-wrap {
  border-radius: 10px;
  border: 1px solid rgba(128, 128, 128, 0.18);
  background: rgba(128, 128, 128, 0.06);
  padding: 2px 12px;
  transition:
    border-color 0.15s ease,
    background 0.15s ease;
}

.settings-field__input-wrap--focused {
  border-color: var(--q-primary, #1976d2);
  background: rgba(128, 128, 128, 0.04);
}

.settings-field__input {
  width: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-family: inherit;
  font-size: 0.88em;
  line-height: 1.4;
  padding: 8px 0;
  color: var(--chat-text);
}

.settings-field__input--sm {
  border: 1px solid rgba(128, 128, 128, 0.18);
  border-radius: 8px;
  padding: 6px 8px;
  flex: 1;
  min-width: 0;
}

.settings-providers__header-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 6px;
}

.settings-providers__advanced-toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-top: 14px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--chat-text-muted);
  font-size: 0.82em;
  cursor: pointer;
}

.settings-providers__advanced-toggle:hover {
  color: var(--chat-text);
}

.settings-providers__set-active-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 18px;
  font-size: 0.85em;
  color: var(--chat-text);
  cursor: pointer;
}

.settings-providers__warning {
  margin-top: 4px;
  font-size: 0.76em;
  color: #f2994a;
}

.settings-providers__submit {
  margin-top: 14px;
}

.settings-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border-radius: 999px;
  border: 1px solid rgba(128, 128, 128, 0.2);
  background: transparent;
  color: var(--chat-text);
  font-size: 0.82em;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.settings-pill:hover:not(:disabled) {
  background: rgba(128, 128, 128, 0.1);
}

.settings-pill:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.settings-pill--danger {
  color: #f44336;
  border-color: rgba(244, 67, 54, 0.3);
}

.settings-pill--danger:hover:not(:disabled) {
  background: rgba(244, 67, 54, 0.1);
}

.settings-pill--primary {
  border-color: transparent;
  background: var(--q-primary, #1976d2);
  color: white;
}

.settings-pill--primary:hover:not(:disabled) {
  filter: brightness(1.08);
}

.settings-provider-picker {
  position: relative;
}

.settings-provider-picker__button {
  width: 100%;
  height: 38px;
  padding: 0 12px;
  border-radius: 10px;
  border: 1px solid rgba(128, 128, 128, 0.18);
  background: rgba(128, 128, 128, 0.06);
  color: var(--chat-text);
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 0.86em;
  cursor: pointer;
}

.settings-provider-picker__button:hover {
  background: rgba(128, 128, 128, 0.1);
}

.settings-provider-picker__dropdown {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  right: 0;
  z-index: 10;
  max-height: 260px;
  overflow-y: auto;
  border-radius: 12px;
  background: rgba(30, 30, 30, 0.9);
  border: 1px solid rgba(128, 128, 128, 0.18);
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
  padding: 6px 0;
}

.settings-provider-picker__group-title {
  font-size: 0.72em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  padding: 6px 14px 4px;
  color: rgba(255, 255, 255, 0.55);
}

.settings-provider-picker__item {
  padding: 8px 14px;
  font-size: 0.86em;
  color: rgba(255, 255, 255, 0.9);
  cursor: pointer;
}

.settings-provider-picker__item:hover {
  background: rgba(255, 255, 255, 0.08);
}

.settings-provider-picker__item--active {
  background: rgba(25, 210, 77, 0.14);
}

.settings-provider-picker__empty {
  display: flex;
  justify-content: center;
  padding: 14px;
}
</style>
