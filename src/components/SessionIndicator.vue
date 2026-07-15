<template>
  <div class="session-indicator" :class="rootClass" :style="rootStyle">
    <span v-if="state === 'idle'" class="session-indicator__idle" aria-hidden="true">
      <Icon icon="mdi:chat-outline" :width="size" :height="size" />
    </span>
    <span v-else-if="state === 'thinking'" class="session-indicator__pulse" aria-hidden="true" />
    <span v-else-if="state === 'writing'" class="session-indicator__bars" aria-hidden="true">
      <span />
      <span />
      <span />
    </span>
    <span v-else-if="state === 'tool'" class="session-indicator__tool" aria-hidden="true">
      <Icon icon="fluent:plug-connected-20-filled" :width="size - 2" :height="size - 2" />
    </span>
    <span v-if="attention" class="session-indicator__attention" aria-hidden="true">
      <Icon icon="line-md:bell-alert-loop" :width="size - 2" :height="size - 2" />
    </span>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { Icon } from "@iconify/vue";

const props = defineProps({
  status: { type: [String, Object], default: null }, // null | 'thinking' | 'writing' | 'sending' | { type:'tool', toolName }
  attention: { type: Boolean, default: false },
  size: { type: Number, default: 16 },
});

const state = computed(() => {
  if (props.attention) return null;
  if (!props.status) return "idle";
  if (typeof props.status === "object" && props.status?.type === "tool") return "tool";
  if (props.status === "thinking") return "thinking";
  if (props.status === "writing" || props.status === "sending") return "writing";
  return "idle";
});

const rootClass = computed(() => ({
  "session-indicator--idle": state.value === "idle" && !props.attention,
  "session-indicator--thinking": state.value === "thinking" && !props.attention,
  "session-indicator--writing": state.value === "writing" && !props.attention,
  "session-indicator--tool": state.value === "tool" && !props.attention,
  "session-indicator--attention": props.attention,
}));

const rootStyle = computed(() => ({
  width: `${props.size}px`,
  height: `${props.size}px`,
}));
</script>

<style scoped>
.session-indicator {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 50%;
  transition:
    background-color 0.25s ease,
    transform 0.25s ease;
}

/* Estado parado: ícone de chat */
.session-indicator__idle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--chat-text-muted, #757575);
  opacity: 0.65;
  transition: opacity 0.25s ease;
}

.session-indicator__idle:hover {
  opacity: 1;
}

.session-indicator--idle {
  color: var(--chat-text-muted, #757575);
}

/* Pensando: pulso suave */
.session-indicator__pulse {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--q-positive, #21ba45);
  animation: session-pulse 1.4s ease-in-out infinite;
}

.session-indicator--thinking {
  color: var(--q-positive, #21ba45);
}

/* Escrevendo/enviando: barras animadas */
.session-indicator__bars {
  display: flex;
  align-items: flex-end;
  justify-content: center;
  gap: 2px;
  height: 10px;
}

.session-indicator__bars span {
  width: 2px;
  height: 4px;
  background: var(--q-positive, #21ba45);
  border-radius: 1px;
  animation: session-bar 0.8s ease-in-out infinite;
}

.session-indicator__bars span:nth-child(2) {
  animation-delay: 0.15s;
}

.session-indicator__bars span:nth-child(3) {
  animation-delay: 0.3s;
}

.session-indicator--writing {
  color: var(--q-positive, #21ba45);
}

/* Ferramenta: ícone de plug */
.session-indicator--tool {
  color: var(--q-info, #31ccec);
}

/* Atenção (permissão pendente): sino animado */
.session-indicator__attention {
  display: inline-flex;
  color: var(--q-warning, #f2c037);
}

.session-indicator--attention {
  color: var(--q-warning, #f2c037);
}

@keyframes session-pulse {
  0%,
  100% {
    transform: scale(0.85);
    opacity: 0.5;
  }
  50% {
    transform: scale(1.35);
    opacity: 1;
  }
}

@keyframes session-bar {
  0%,
  100% {
    height: 4px;
  }
  50% {
    height: 10px;
  }
}
</style>
