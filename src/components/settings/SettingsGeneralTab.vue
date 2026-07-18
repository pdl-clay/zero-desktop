<template>
  <div class="settings-general">
    <!-- Language -->
    <section class="settings-general__section">
      <div class="settings-general__section-title">{{ $t("settings.languageLabel") }}</div>
      <div class="settings-general__pills">
        <button
          type="button"
          class="settings-pill"
          :class="{ 'settings-pill--active': currentLocale === 'pt-BR' }"
          @click="setLocale('pt-BR')"
        >
          {{ $t("settings.languagePtBR") }}
        </button>
        <button
          type="button"
          class="settings-pill"
          :class="{ 'settings-pill--active': currentLocale === 'en-US' }"
          @click="setLocale('en-US')"
        >
          {{ $t("settings.languageEnUS") }}
        </button>
      </div>
    </section>

    <!-- Default advisor config -->
    <section class="settings-general__section">
      <div class="settings-general__section-title">{{ $t("settings.advisorDefaultTitle") }}</div>
      <div class="settings-general__row">
        <q-toggle
          v-model="advisorConfig.enabled"
          color="primary"
          @update:model-value="saveAdvisorConfig"
        />
        <span class="settings-general__row-label">{{ $t("chat.advisorSettings") }}</span>
      </div>

      <template v-if="advisorConfig.enabled">
        <div class="settings-general__field">
          <span class="settings-field__label">{{ $t("chat.advisorModelLabel") }}</span>
          <ModelPickerDropdown
            v-model="advisorConfig.model"
            :placeholder-label="$t('chat.advisorModelDefault')"
            :title="$t('chat.advisorModelLabel')"
            allow-clear
            @update:model-value="saveAdvisorConfig"
          />
        </div>

        <div class="settings-general__field">
          <span class="settings-field__label">{{ $t("chat.advisorTriggerModeLabel") }}</span>
          <div class="settings-general__pills">
            <button
              type="button"
              class="settings-pill"
              :class="{ 'settings-pill--active': advisorConfig.mode === 'max' }"
              @click="setAdvisorMode('max')"
            >
              {{ $t("chat.advisorModeMax") }}
              <q-tooltip>{{ $t("chat.advisorModeMaxTooltip") }}</q-tooltip>
            </button>
            <button
              type="button"
              class="settings-pill"
              :class="{ 'settings-pill--active': advisorConfig.mode === 'low' }"
              @click="setAdvisorMode('low')"
            >
              {{ $t("chat.advisorModeLow") }}
              <q-tooltip>{{ $t("chat.advisorModeLowTooltip") }}</q-tooltip>
            </button>
          </div>
        </div>
      </template>
    </section>

    <!-- About -->
    <section class="settings-general__section">
      <div class="settings-general__section-title">{{ $t("settings.aboutTitle") }}</div>
      <div class="settings-general__about-row">
        <span class="settings-general__about-label">{{ $t("settings.zeroPathLabel") }}</span>
        <span class="settings-general__about-value">{{ zeroStore.zeroPath || "—" }}</span>
      </div>
      <div class="settings-general__about-row">
        <span class="settings-general__about-label">{{ $t("settings.zeroVersionLabel") }}</span>
        <span class="settings-general__about-value">{{ zeroStore.zeroVersion || "—" }}</span>
      </div>
      <button
        type="button"
        class="settings-pill settings-general__recheck-btn"
        @click="zeroStore.locateZero()"
      >
        <q-icon name="refresh" size="14px" />
        {{ $t("settings.recheck") }}
      </button>

      <div class="settings-general__about-row">
        <span class="settings-general__about-label">{{ $t("settings.appVersionLabel") }}</span>
        <span class="settings-general__about-value">{{ zeroStore.appVersion || "—" }}</span>
      </div>
      <template v-if="zeroStore.isAppImageRuntime">
        <button
          type="button"
          class="settings-pill settings-general__recheck-btn"
          :disabled="zeroStore.isCheckingUpdate || zeroStore.isDownloadingUpdate"
          @click="onCheckForUpdates"
        >
          <q-icon name="cloud_sync" size="14px" />
          {{ $t("settings.checkForUpdates") }}
        </button>
        <div class="settings-general__update-status" v-if="updateStatusText">
          {{ updateStatusText }}
        </div>
        <button
          v-if="zeroStore.updateReadyToInstall"
          type="button"
          class="settings-pill settings-general__recheck-btn"
          @click="zeroStore.restartToApplyUpdate()"
        >
          {{ $t("settings.restartNow") }}
        </button>
      </template>
    </section>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { i18n, LOCALE_STORAGE_KEY } from "@/i18n/instance";
import { useZeroStore } from "@/stores/zero-store";
import ModelPickerDropdown from "@/components/chat/ModelPickerDropdown.vue";

const { t } = useI18n();
const zeroStore = useZeroStore();

const currentLocale = ref(i18n.global.locale.value);

function setLocale(locale) {
  i18n.global.locale.value = locale;
  currentLocale.value = locale;
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    // localStorage unavailable - the choice just won't survive a reload.
  }
}

const advisorConfig = ref({ enabled: false, model: null, mode: "max" });

onMounted(async () => {
  advisorConfig.value = await zeroStore.loadDefaultAdvisorConfig({ force: true });
  await zeroStore.loadAppInfo();
});

async function saveAdvisorConfig() {
  await zeroStore.saveDefaultAdvisorConfig(advisorConfig.value);
}

function setAdvisorMode(mode) {
  advisorConfig.value.mode = mode;
  saveAdvisorConfig();
}

const hasCheckedForUpdates = ref(false);

async function onCheckForUpdates() {
  await zeroStore.checkForUpdates({ silent: true });
  hasCheckedForUpdates.value = true;
  if (zeroStore.updateAvailable) {
    await zeroStore.downloadAndInstallUpdate();
  }
}

const updateStatusText = computed(() => {
  if (zeroStore.updateError) return t("settings.updateCheckFailed");
  if (zeroStore.updateReadyToInstall) return t("settings.updateReadyToRestart");
  if (zeroStore.isDownloadingUpdate) return t("settings.updateDownloading");
  if (zeroStore.isCheckingUpdate) return t("settings.checkingForUpdates");
  if (zeroStore.updateAvailable && zeroStore.updateInfo) {
    return t("settings.updateAvailable", { version: zeroStore.updateInfo.version });
  }
  if (hasCheckedForUpdates.value) return t("settings.updateUpToDate");
  return "";
});
</script>

<style scoped>
.settings-general {
  display: flex;
  flex-direction: column;
  gap: 26px;
}

.settings-general__section-title {
  font-size: 0.75em;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: var(--chat-text-muted);
  margin-bottom: 10px;
}

.settings-general__row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.settings-general__row-label {
  font-size: 0.88em;
  color: var(--chat-text);
}

.settings-general__field {
  margin-top: 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.settings-field__label {
  font-size: 0.8em;
  color: var(--chat-text-muted);
}

.settings-general__pills {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
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
  font-size: 0.85em;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.settings-pill:hover {
  background: rgba(128, 128, 128, 0.1);
}

.settings-pill--active {
  border-color: rgba(25, 210, 77, 0.4);
  background: rgba(25, 210, 77, 0.1);
}

.settings-general__about-row {
  display: flex;
  gap: 8px;
  font-size: 0.84em;
  padding: 4px 0;
}

.settings-general__about-label {
  flex-shrink: 0;
  width: 130px;
  color: var(--chat-text-muted);
}

.settings-general__about-value {
  color: var(--chat-text);
  overflow-wrap: anywhere;
}

.settings-general__recheck-btn {
  margin-top: 10px;
}

.settings-general__update-status {
  margin-top: 8px;
  font-size: 0.82em;
  color: var(--chat-text-muted);
}
</style>
