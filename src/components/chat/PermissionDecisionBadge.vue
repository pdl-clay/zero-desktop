<template>
  <div class="permission-decision row items-start q-mb-sm">
    <q-icon :name="iconName" :color="iconColor" size="14px" class="q-mr-xs q-mt-xs" />
    <div class="text-caption text-grey-7">
      <span class="text-weight-medium">{{ message.toolName }}</span>
      —
      <span>{{ statusText }}</span>
      <span v-if="message.reason">: {{ message.reason }}</span>
      <q-badge
        v-if="message.riskLevel"
        outline
        :color="riskColor"
        class="q-ml-xs"
        :label="message.riskLevel"
      />
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps({
  message: { type: Object, required: true },
});

const { t: $t } = useI18n();

const isExpired = computed(() => props.message.action === "expired");

const allowed = computed(() => props.message.action !== "deny" && !isExpired.value);

const iconName = computed(() => {
  if (isExpired.value) return "schedule";
  return allowed.value ? "shield" : "block";
});

const iconColor = computed(() => {
  if (isExpired.value) return "grey-6";
  return allowed.value ? "positive" : "negative";
});

const statusText = computed(() => {
  if (isExpired.value) return $t("chat.decisionExpired");
  return allowed.value ? $t("chat.decisionAllowed") : $t("chat.decisionDenied");
});

const riskColor = computed(() => {
  switch (props.message.riskLevel) {
    case "high":
      return "negative";
    case "medium":
      return "warning";
    default:
      return "grey-6";
  }
});
</script>

<style scoped>
.permission-decision {
  line-height: 1.4;
}
</style>
