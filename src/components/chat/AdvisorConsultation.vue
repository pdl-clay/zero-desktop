<template>
  <div class="advisor-consultation q-mb-md">
    <div class="advisor-header q-pa-sm">
      <q-icon name="auto_awesome" size="16px" class="q-mr-xs" />
      <span class="text-subtitle2">Advisor Consultation</span>
    </div>
    <div class="advisor-content q-pa-md">
      <div v-html="renderedContent" />
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { renderMarkdown } from "@/utils/markdown";

const props = defineProps({
  message: {
    type: Object,
    required: true,
  },
});

const renderedContent = computed(() => {
  return renderMarkdown(props.message.content || "");
});
</script>

<style scoped>
.advisor-consultation {
  border: 1px solid rgba(167, 139, 250, 0.3);
  border-radius: 8px;
  background: linear-gradient(135deg, rgba(167, 139, 250, 0.08), rgba(251, 191, 36, 0.08));
  overflow: hidden;
}

.advisor-header {
  background: linear-gradient(135deg, rgba(167, 139, 250, 0.15), rgba(251, 191, 36, 0.15));
  border-bottom: 1px solid rgba(167, 139, 250, 0.2);
  display: flex;
  align-items: center;
  color: #a78bfa;
}

.advisor-content {
  font-size: 0.95em;
  line-height: 1.6;
}

.advisor-content :deep(p) {
  margin-bottom: 0.5em;
}

.advisor-content :deep(ul),
.advisor-content :deep(ol) {
  margin-left: 1.5em;
  margin-bottom: 0.5em;
}

.advisor-content :deep(strong) {
  color: #a78bfa;
}
</style>
