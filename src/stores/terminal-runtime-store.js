import { defineStore, acceptHMRUpdate } from "pinia";
import { useTerminalSessionStore } from "@/stores/terminal-session-store";

const PANEL_HEIGHT_KEY = "zero-desktop-terminal-panel-height";
const DEFAULT_PANEL_HEIGHT = 320;
const MIN_PANEL_HEIGHT = 160;

function loadPanelHeight() {
  const raw = localStorage.getItem(PANEL_HEIGHT_KEY);
  const parsed = raw ? parseInt(raw, 10) : NaN;
  return Number.isFinite(parsed) && parsed >= MIN_PANEL_HEIGHT ? parsed : DEFAULT_PANEL_HEIGHT;
}

// Terminal tabs are scoped per workspace, the same way chat session panels
// are (session-runtime-store.js) - each workspace has its own set of open
// terminals, hidden/shown when switching the active workspace. Unlike chat
// panels there is no cap and no "hide but keep running" close mode: a
// terminal tab's whole reason for existing is its live shell, so closing
// one always kills the process, exactly like closing a real terminal window.
export const useTerminalRuntimeStore = defineStore("terminal-runtime", {
  state: () => ({
    openKeys: [],
    focusedKeyByPath: {},
    keyMeta: {},
    panelOpen: false,
    panelHeightPx: loadPanelHeight(),
  }),

  getters: {
    isOpen: (state) => (key) => state.openKeys.includes(key),
    visibleKeys: (state) => (workspacePath) =>
      state.openKeys.filter((k) => state.keyMeta[k]?.cwd === workspacePath),
    focusedKeyFor: (state) => (workspacePath) => state.focusedKeyByPath[workspacePath] ?? null,
  },

  actions: {
    registerMeta(key, patch) {
      this.keyMeta[key] = { ...this.keyMeta[key], ...patch };
    },

    setPanelHeight(px) {
      const max = Math.round(window.innerHeight * 0.8);
      const clamped = Math.min(Math.max(px, MIN_PANEL_HEIGHT), max);
      this.panelHeightPx = clamped;
      localStorage.setItem(PANEL_HEIGHT_KEY, String(clamped));
    },

    openTab(key, workspacePath) {
      if (!this.openKeys.includes(key)) {
        this.openKeys.push(key);
        this.registerMeta(key, { cwd: workspacePath, status: "spawning" });
      }
      this.focusedKeyByPath[workspacePath] = key;
      this.panelOpen = true;
    },

    focusTab(key, workspacePath) {
      if (this.openKeys.includes(key)) {
        this.focusedKeyByPath[workspacePath] = key;
      }
    },

    // Shared close-bookkeeping: drops the key from openKeys/keyMeta and
    // reassigns focus for whichever workspace it belonged to. Reads cwd off
    // keyMeta itself, so it must run before keyMeta[key] is deleted anywhere
    // else.
    _removeFromOpen(key) {
      const workspacePath = this.keyMeta[key]?.cwd;
      this.openKeys = this.openKeys.filter((k) => k !== key);
      delete this.keyMeta[key];
      if (workspacePath && this.focusedKeyByPath[workspacePath] === key) {
        const remaining = this.visibleKeys(workspacePath);
        this.focusedKeyByPath[workspacePath] = remaining.at(-1) ?? null;
      }
    },

    async closeTab(key) {
      const store = useTerminalSessionStore(key);
      await store.kill();
      store.$reset();
      store.$dispose();
      this._removeFromOpen(key);
    },
  },
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useTerminalRuntimeStore, import.meta.hot));
}
