<template>
  <div v-if="status" :class="['working-indicator row items-center q-px-md q-py-xs', statusClass]">
    <q-spinner-dots v-if="status === 'thinking'" size="14px" color="amber" />
    <q-spinner-dots v-else-if="isTool" size="14px" color="info" />
    <q-spinner-dots v-else-if="status === 'sending'" size="14px" color="grey" />
    <q-spinner-dots v-else size="14px" color="positive" />
    <span class="q-ml-sm text-caption text-weight-medium text-grey-7">{{ statusLabel }}</span>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";

const { t: $t } = useI18n();
const zeroStore = useZeroStore();

const status = computed(() => zeroStore.workingStatus);

const isTool = computed(() => typeof status.value === "object" && status.value.type === "tool");

const statusClass = computed(() => {
  if (!status.value) return "";
  if (status.value === "thinking") return "working-indicator--thinking";
  if (isTool.value) return "working-indicator--tool";
  if (status.value === "sending") return "working-indicator--sending";
  return "working-indicator--writing";
});

const statusLabel = computed(() => {
  if (status.value === "thinking") return $t("chat.thinkingRunning");
  if (isTool.value) return `${$t("chat.toolRunning")} ${status.value.toolName}...`;
  if (status.value === "sending") return $t("chat.sending");
  if (status.value === "writing") return $t("chat.writing");
  return "";
});
</script>

<style scoped>
.working-indicator {
  border-bottom: 1px solid transparent;
  min-height: 28px;
}
.working-indicator--thinking {
  border-bottom-color: rgba(255, 193, 7, 0.25);
  background: rgba(255, 193, 7, 0.05);
}
.working-indicator--tool {
  border-bottom-color: rgba(33, 150, 243, 0.25);
  background: rgba(33, 150, 243, 0.05);
}
.working-indicator--writing {
  border-bottom-color: rgba(76, 175, 80, 0.25);
  background: rgba(76, 175, 80, 0.05);
}
.working-indicator--sending {
  border-bottom-color: rgba(158, 158, 158, 0.25);
  background: rgba(158, 158, 158, 0.05);
}
</style>
