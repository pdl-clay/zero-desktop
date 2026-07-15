<template>
  <Icon
    v-if="status === 'working'"
    icon="svg-spinners:3-dots-bounce"
    :width="size"
    :height="size"
    class="status-badge status-badge--working"
  />
  <Icon
    v-else-if="status === 'attention'"
    icon="line-md:bell-alert-loop"
    :width="size"
    :height="size"
    class="status-badge status-badge--attention"
  />
</template>

<script setup>
import { Icon } from "@iconify/vue";

defineProps({
  status: { type: String, default: null }, // "working" | "attention" | null
  size: { type: [String, Number], default: 14 },
});
</script>

<style scoped>
.status-badge--working {
  color: var(--q-positive, #21ba45);
}
.status-badge--attention {
  color: var(--q-warning, #f2c037);
}
</style>
