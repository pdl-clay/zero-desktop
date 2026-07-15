<template>
  <div class="plan-panel">
    <div class="plan-panel__header row items-center justify-between">
      <span class="plan-panel__title">{{ $t("plan.title") }}</span>
      <q-icon name="checklist" size="18px" color="grey-6" />
    </div>
    <q-separator class="q-my-sm" />
    <q-scroll-area class="plan-panel__scroll">
      <div v-for="(item, i) in plan" :key="i" class="plan-panel__item row items-start">
        <q-icon
          :name="planIcon(item.status)"
          :color="planColor(item.status)"
          size="16px"
          class="q-mr-sm q-mt-xs"
        />
        <span
          :class="['plan-panel__text', item.status === 'completed' ? 'plan-panel__text--done' : '']"
        >
          {{ item.content }}
        </span>
      </div>
    </q-scroll-area>
  </div>
</template>

<script setup>
import { planIcon, planColor } from "@/utils/plan";

defineProps({
  plan: { type: Array, required: true },
});
</script>

<style scoped>
.plan-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px;
}

.plan-panel__header {
  padding: 0 4px;
}

.plan-panel__title {
  font-size: 0.75em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.8));
}

.plan-panel__scroll {
  flex: 1;
  min-height: 0;
}

.plan-panel__item {
  padding: 6px 4px;
}

.plan-panel__text {
  font-size: 0.85em;
  line-height: 1.4;
  color: var(--chat-text);
}

.plan-panel__text--done {
  color: var(--chat-text-muted);
  text-decoration: line-through;
}
</style>
