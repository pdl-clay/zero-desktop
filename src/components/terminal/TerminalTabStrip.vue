<template>
  <div class="terminal-tab-strip">
    <button
      v-for="key in tabs"
      :key="key"
      type="button"
      class="terminal-tab-chip"
      :class="{
        'terminal-tab-chip--active': key === activeKey,
        'terminal-tab-chip--exited': keyMeta[key]?.status === 'exited',
      }"
      @click="$emit('focus', key)"
    >
      <span
        class="terminal-tab-chip__dot"
        :class="'terminal-tab-chip__dot--' + (keyMeta[key]?.status || 'spawning')"
      />
      <span class="terminal-tab-chip__name">{{ keyMeta[key]?.title || $t("terminal.defaultTitle") }}</span>
      <q-icon
        name="close"
        size="13px"
        class="terminal-tab-chip__close"
        @click.stop="$emit('close', key)"
      >
        <q-tooltip>{{ $t("terminal.closeTab") }}</q-tooltip>
      </q-icon>
    </button>
    <button type="button" class="terminal-tab-add" @click="$emit('new-tab')">
      <q-icon name="add" size="16px" />
      <q-tooltip>{{ $t("terminal.newTab") }}</q-tooltip>
    </button>
  </div>
</template>

<script setup>
defineProps({
  tabs: {
    type: Array,
    required: true,
  },
  activeKey: {
    type: String,
    default: null,
  },
  keyMeta: {
    type: Object,
    required: true,
  },
});

defineEmits(["focus", "close", "new-tab"]);
</script>

<style scoped>
.terminal-tab-strip {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  overflow-x: auto;
  gap: 6px;
  padding: 6px 8px;
  flex: 1;
  min-width: 0;
}

.terminal-tab-chip {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 180px;
  padding: 5px 8px;
  border-radius: 999px;
  border: 1px solid rgba(128, 128, 128, 0.18);
  background: rgba(128, 128, 128, 0.06);
  color: var(--chat-text);
  font-size: 0.8em;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.terminal-tab-chip:hover {
  background: rgba(128, 128, 128, 0.12);
}

.terminal-tab-chip--active {
  border-color: rgba(25, 210, 77, 0.4);
  background: rgba(25, 210, 77, 0.1);
}

.terminal-tab-chip--exited {
  opacity: 0.6;
}

.terminal-tab-chip__dot {
  flex-shrink: 0;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: rgba(128, 128, 128, 0.5);
}

.terminal-tab-chip__dot--running {
  background: #21ba45;
}

.terminal-tab-chip__dot--spawning {
  background: #f2c037;
}

.terminal-tab-chip__dot--exited {
  background: #f44336;
}

.terminal-tab-chip__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.terminal-tab-chip__close {
  flex-shrink: 0;
  border-radius: 50%;
  color: rgba(128, 128, 128, 0.75);
  transition:
    background 0.15s ease,
    color 0.15s ease;
}

.terminal-tab-chip__close:hover {
  background: rgba(244, 67, 54, 0.16);
  color: #f44336;
}

.terminal-tab-add {
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: rgba(128, 128, 128, 0.85);
  cursor: pointer;
  transition:
    background 0.15s ease,
    color 0.15s ease;
}

.terminal-tab-add:hover {
  background: rgba(25, 210, 77, 0.14);
  color: #19d24d;
}
</style>
