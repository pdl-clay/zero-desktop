<template>
  <div class="permission-decision row items-start q-mb-sm">
    <q-icon
      :name="allowed ? 'shield' : 'block'"
      :color="allowed ? 'positive' : 'negative'"
      size="14px"
      class="q-mr-xs q-mt-xs"
    />
    <div class="text-caption text-grey-7">
      <span class="text-weight-medium">{{ message.toolName }}</span>
      —
      <span>{{ allowed ? $t("chat.decisionAllowed") : $t("chat.decisionDenied") }}</span>
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

const allowed = computed(() => props.message.action !== "deny");

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
