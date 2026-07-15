<template>
  <div
    class="workspace-avatar cursor-pointer"
    :class="{
      'workspace-avatar--active': isActive,
      'workspace-avatar--working': isWorking,
    }"
    :style="avatarStyle"
    @click="$emit('click', $event)"
  >
    <span class="workspace-avatar__letter">{{ letter }}</span>
    <span class="workspace-avatar__ring" aria-hidden="true" />
    <StatusBadge
      v-if="status === 'attention'"
      :status="status"
      :size="badgeSize"
      class="workspace-avatar__badge"
    />
  </div>
</template>

<script setup>
import { computed } from "vue";
import StatusBadge from "./StatusBadge.vue";

const props = defineProps({
  name: { type: String, required: true },
  color: { type: String, required: true },
  isActive: { type: Boolean, default: false },
  isWorking: { type: Boolean, default: false },
  status: { type: String, default: null }, // "attention" | null
  badgeSize: { type: [String, Number], default: 13 },
});

defineEmits(["click"]);

const letter = computed(() => props.name.charAt(0).toUpperCase());

const avatarStyle = computed(() => {
  const size = props.isActive ? 40 : 34;
  const fontSize = props.isActive ? 16 : 12;
  return {
    backgroundColor: props.color,
    width: `${size}px`,
    height: `${size}px`,
    fontSize: `${fontSize}px`,
  };
});
</script>

<style scoped>
.workspace-avatar {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  color: #fff;
  font-weight: 700;
  line-height: 1;
  transition: all 0.2s ease;
  user-select: none;
  flex-shrink: 0;
  overflow: visible;
}

.workspace-avatar:hover {
  opacity: 0.85;
  transform: scale(1.12);
}

.workspace-avatar--active {
  opacity: 1;
  transform: scale(1);
}

/* Ring só aparece quando há algum agente trabalhando no workspace */
.workspace-avatar__ring {
  position: absolute;
  inset: -4px;
  border-radius: 50%;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.25s ease;
}

.workspace-avatar--working .workspace-avatar__ring {
  opacity: 1;
  border: 2px dashed #fff;
  animation: workspace-spin-clockwise 2.2s linear infinite;
}

.workspace-avatar__badge {
  position: absolute;
  bottom: -2px;
  right: -2px;
  z-index: 2;
}

.workspace-avatar__letter {
  position: relative;
  z-index: 1;
}

@keyframes workspace-spin-clockwise {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
