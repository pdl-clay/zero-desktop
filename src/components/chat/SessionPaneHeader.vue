<template>
  <div
    ref="headerRef"
    :class="[
      'session-pane-header row items-center justify-between q-px-sm q-py-xs',
      { 'session-pane-header--narrow': isNarrow },
    ]"
  >
    <div class="row items-center q-gutter-x-sm" style="min-width: 0">
      <StatusBadge v-if="meta?.hasPendingPermission" status="attention" size="16" />
      <span class="session-pane-header__title text-caption text-weight-medium ellipsis">
        {{ title }}
      </span>
    </div>

    <div class="row items-center q-gutter-x-xs">
      <q-btn
        flat
        dense
        round
        size="sm"
        icon="close"
        color="grey-6"
        class="session-pane-header__btn"
        @click="onClose"
      >
        <q-tooltip>{{ $t("workspace.closePanel") }}</q-tooltip>
      </q-btn>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, onMounted, onUnmounted } from "vue";
import { useSessionRuntimeStore } from "@/stores/session-runtime-store";
import StatusBadge from "@/components/StatusBadge.vue";

const NARROW_THRESHOLD = 500;

const props = defineProps({
  sessionKey: { type: String, required: true },
});

const runtime = useSessionRuntimeStore();
const meta = computed(() => runtime.keyMeta[props.sessionKey] || {});

const headerRef = ref(null);
const isNarrow = ref(false);
let resizeObserver = null;

onMounted(() => {
  if (headerRef.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        isNarrow.value = entry.contentRect.width < NARROW_THRESHOLD;
      }
    });
    resizeObserver.observe(headerRef.value);
  }
});

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect();
    resizeObserver = null;
  }
});

const title = computed(() => {
  const m = meta.value;
  if (m.title) return m.title;
  if (m.sessionId) return m.sessionId.slice(-8);
  return "...";
});

async function onClose() {
  await runtime.closePanel(props.sessionKey);
}
</script>

<style scoped>
.session-pane-header {
  border-bottom: 1px solid rgba(128, 128, 128, 0.16);
  min-height: 34px;
}
.session-pane-header--narrow .session-pane-header__title {
  max-width: 80px;
}
.session-pane-header__title {
  color: var(--chat-text);
  max-width: 160px;
}

.session-pane-header__btn {
  background: rgba(128, 128, 128, 0.12);
  transition:
    background 0.15s ease,
    transform 0.1s ease;
}

.session-pane-header__btn:hover {
  background: rgba(128, 128, 128, 0.2);
  transform: scale(1.06);
}

.session-pane-header__btn:active {
  transform: scale(0.94);
}
</style>
