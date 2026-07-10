<template>
  <div class="error-card q-mb-sm">
    <div class="row items-start q-px-sm q-py-sm">
      <q-icon name="error_outline" color="negative" size="18px" class="q-mr-sm q-mt-xs" />
      <span class="text-body2 error-content">{{ message.content }}</span>
    </div>
  </div>
</template>

<script setup>
defineProps({
  message: { type: Object, required: true },
});
</script>

<style scoped>
.error-card {
  border-radius: 8px;
  border: 1px solid rgba(244, 67, 54, 0.3);
  background: rgba(244, 67, 54, 0.06);
  color: var(--chat-text);
}
body.body--dark .error-card {
  border-color: rgba(244, 67, 54, 0.25);
  background: rgba(244, 67, 54, 0.05);
}
.error-content {
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
