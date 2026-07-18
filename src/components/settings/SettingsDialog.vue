<template>
  <q-dialog
    :model-value="modelValue"
    @update:model-value="onUpdate"
    transition-show="scale"
    transition-hide="scale"
  >
    <div class="settings-dialog" :class="{ 'settings-dialog--dark': $q.dark.isActive }">
      <div class="settings-dialog__header">
        <div class="settings-dialog__header-title">
          <q-icon name="settings" size="18px" class="q-mr-sm" color="primary" />
          {{ $t("settings.title") }}
        </div>
        <button type="button" class="settings-dialog__header-btn" @click="onUpdate(false)">
          <q-icon name="close" size="18px" />
          <q-tooltip>{{ $t("common.close") }}</q-tooltip>
        </button>
      </div>

      <div class="settings-dialog__tabs">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          type="button"
          class="settings-tab-chip"
          :class="{ 'settings-tab-chip--active': activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          <q-icon :name="tab.icon" size="15px" />
          {{ tab.label }}
        </button>
      </div>

      <div class="settings-dialog__body">
        <SettingsGeneralTab v-if="activeTab === 'general'" />
        <SettingsProvidersTab v-else />
      </div>
    </div>
  </q-dialog>
</template>

<script setup>
import { ref, computed, watch } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";
import SettingsGeneralTab from "./SettingsGeneralTab.vue";
import SettingsProvidersTab from "./SettingsProvidersTab.vue";

const $q = useQuasar();
const { t } = useI18n();
const zeroStore = useZeroStore();

const props = defineProps({
  modelValue: {
    type: Boolean,
    default: false,
  },
});
const emit = defineEmits(["update:modelValue"]);

function onUpdate(value) {
  emit("update:modelValue", value);
}

const activeTab = ref("general");
const tabs = computed(() => [
  { id: "general", label: t("settings.tabGeneral"), icon: "tune" },
  { id: "providers", label: t("settings.tabProviders"), icon: "dns" },
]);

// Reload provider data every time the dialog opens - the catalog is cached
// (static per zero CLI version) but the configured-providers list is forced
// fresh since it can have changed from outside this dialog (e.g. the user
// ran `zero providers add` in a terminal since the last time it was open).
watch(
  () => props.modelValue,
  (open) => {
    if (!open) return;
    activeTab.value = "general";
    zeroStore.loadProviderCatalog();
    zeroStore.loadConfiguredProviders({ force: true });
  },
);
</script>

<style scoped>
.settings-dialog {
  width: 620px;
  max-width: 92vw;
  max-height: 82vh;
  display: flex;
  flex-direction: column;
  border-radius: 20px;
  /* --chat-card-bg is a near-transparent tint (0.035/0.045 alpha) meant to
     sit on top of a Quasar-provided opaque surface (q-drawer/q-card already
     paint a solid background beneath it) - this dialog is a plain floating
     div with nothing opaque behind it, so that token alone left it looking
     washed-out/see-through. Solid surface colors here instead, matching the
     app's light/dark backgrounds directly. */
  background: rgb(248, 248, 249);
  border: 1px solid var(--chat-card-border, rgba(128, 128, 128, 0.16));
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.22);
  overflow: hidden;
}

.settings-dialog--dark {
  background: rgb(30, 30, 32);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
}

.settings-dialog__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 20px 14px;
  border-bottom: 1px solid rgba(128, 128, 128, 0.14);
  flex-shrink: 0;
}

.settings-dialog__header-title {
  display: flex;
  align-items: center;
  font-size: 0.98em;
  font-weight: 700;
  color: var(--chat-text);
  letter-spacing: 0.2px;
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

.settings-dialog__header-btn:hover {
  background: rgba(128, 128, 128, 0.14);
  color: var(--chat-text);
  transform: scale(1.06);
}

.settings-dialog__header-btn:active {
  transform: scale(0.94);
}

.settings-dialog__tabs {
  display: flex;
  gap: 8px;
  padding: 12px 20px;
  border-bottom: 1px solid rgba(128, 128, 128, 0.14);
  flex-shrink: 0;
}

.settings-tab-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border-radius: 999px;
  border: 1px solid rgba(128, 128, 128, 0.18);
  background: rgba(128, 128, 128, 0.06);
  color: var(--chat-text);
  font-size: 0.84em;
  font-weight: 500;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.settings-tab-chip:hover {
  background: rgba(128, 128, 128, 0.12);
}

.settings-tab-chip--active {
  border-color: rgba(25, 210, 77, 0.4);
  background: rgba(25, 210, 77, 0.1);
}

.settings-dialog__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 20px;
}
</style>
