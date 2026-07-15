<template>
  <div ref="hostEl" class="terminal-host"></div>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount, watch } from "vue";
import { useQuasar } from "quasar";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { useTerminalSessionStore } from "@/stores/terminal-session-store";

const props = defineProps({
  terminalKey: {
    type: String,
    required: true,
  },
  cwd: {
    type: String,
    required: true,
  },
});

const $q = useQuasar();
const store = useTerminalSessionStore(props.terminalKey);
const hostEl = ref(null);

const DARK_THEME = {
  background: "#1e1e1e",
  foreground: "#e0e0e0",
  cursor: "#19d24d",
  selectionBackground: "rgba(25, 210, 77, 0.3)",
};
const LIGHT_THEME = {
  background: "#fafafa",
  foreground: "#1a1a1a",
  cursor: "#19d24d",
  selectionBackground: "rgba(25, 210, 77, 0.25)",
};

watch(
  () => $q.dark.isActive,
  (isDark) => {
    if (store.term) store.term.options.theme = isDark ? DARK_THEME : LIGHT_THEME;
  },
);

let resizeObserver = null;
let resizeTimer = null;

onMounted(() => {
  // This component only ever unmounts when its tab is actually closed (the
  // terminal panel keeps every open tab's host alive via v-show, across
  // both tab switches and workspace switches, so scrollback is never lost)
  // - so onMounted firing with a terminal already attached would only ever
  // happen from a dev-time HMR remount, not real usage. Skip re-creating it
  // rather than trying to re-open the same xterm.js instance into a new
  // element, which isn't a supported path.
  if (!store.term) {
    const term = new Terminal({
      fontFamily: '"JetBrains Mono", "Fira Code", Consolas, monospace',
      fontSize: 13,
      scrollback: 5000,
      allowProposedApi: true,
      theme: $q.dark.isActive ? DARK_THEME : LIGHT_THEME,
    });
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(hostEl.value);
    fitAddon.fit();
    store.attach(term, fitAddon);
    store.spawn(props.cwd);
  }

  resizeObserver = new ResizeObserver(() => {
    if (!store.fitAddon) return;
    store.fitAddon.fit();
    // Debounced: a live window/panel drag-resize fires this continuously,
    // and each resize is a round trip to the backend to ioctl the pty.
    if (resizeTimer) clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      store.resize(store.term.cols, store.term.rows);
    }, 80);
  });
  resizeObserver.observe(hostEl.value);
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
  if (resizeTimer) clearTimeout(resizeTimer);
});
</script>

<style scoped>
.terminal-host {
  width: 100%;
  height: 100%;
  overflow: hidden;
  padding: 4px 8px;
  box-sizing: border-box;
}

.terminal-host :deep(.xterm) {
  height: 100%;
}
</style>
