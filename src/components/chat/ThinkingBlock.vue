<template>
  <!-- Streaming: thin bar, not expandable -->
  <div
    v-if="streaming"
    :class="[
      'thinking-bar row items-center q-px-sm q-mb-sm',
      $q.dark.isActive ? 'thinking-bar--dark' : '',
    ]"
  >
    <q-spinner-dots size="14px" color="amber" class="q-mr-sm" />
    <span class="text-caption text-grey-6">{{ $t("chat.thinkingRunning") }}</span>
  </div>

  <!-- Finalized: collapsible expansion item -->
  <div v-else :class="['thinking-block q-mb-sm', $q.dark.isActive ? 'thinking-block--dark' : '']">
    <q-expansion-item v-model="expanded" dense dense-toggle :header-style="headerStyle">
      <template v-slot:header>
        <q-item-section avatar>
          <q-icon :name="expanded ? 'psychology' : 'psychology_alt'" size="16px" />
        </q-item-section>
        <q-item-section>
          <q-item-label class="text-caption text-weight-medium text-grey-6">
            {{ $t("chat.thinking") }}
          </q-item-label>
        </q-item-section>
        <q-item-section side>
          <q-icon name="check" size="14px" color="grey-5" />
        </q-item-section>
      </template>
      <div :class="['thinking-content q-pa-sm', $q.dark.isActive ? 'text-grey-4' : 'text-grey-8']">
        {{ content }}
      </div>
    </q-expansion-item>
  </div>
</template>

<script setup>
import { ref, computed } from "vue";
import { useQuasar } from "quasar";

const props = defineProps({
  message: { type: Object, required: true },
  streaming: { type: Boolean, default: false },
});

const $q = useQuasar();
const expanded = ref(false);

const content = computed(() => props.message?.content || "");

const headerStyle = computed(() => ({
  padding: "2px 6px",
  minHeight: "28px",
  background: $q.dark.isActive ? "rgba(255, 193, 7, 0.04)" : "rgba(255, 193, 7, 0.08)",
  borderRadius: "6px",
}));
</script>

<style scoped>
.thinking-bar {
  border-radius: 6px;
  border: 1px solid rgba(255, 193, 7, 0.18);
  background: rgba(255, 193, 7, 0.05);
  min-height: 28px;
}
.thinking-bar--dark {
  border-color: rgba(255, 193, 7, 0.12);
  background: rgba(255, 193, 7, 0.03);
}
.thinking-block :deep(.thinking-header) {
  border-radius: 6px;
  transition: background 0.2s ease;
}
.thinking-block :deep(.thinking-header:hover) {
  background: rgba(255, 193, 7, 0.12) !important;
}
.thinking-content {
  font-style: italic;
  font-size: 0.85em;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.5;
}
</style>
