import { defineStore, acceptHMRUpdate } from "pinia";
import { useZeroSessionStore } from "@/stores/zero-session-store";

export const MAX_OPEN_PANELS = 4;

export const useSessionRuntimeStore = defineStore("session-runtime", {
  state: () => ({
    openKeys: [],
    focusedKey: null,
    keyMeta: {},
  }),

  getters: {
    canOpenMore: (state) => state.openKeys.length < MAX_OPEN_PANELS,
    isOpen: (state) => (key) => state.openKeys.includes(key),
  },

  actions: {
    registerMeta(key, patch) {
      this.keyMeta[key] = { ...this.keyMeta[key], ...patch };
    },

    openPanel(key) {
      if (!this.openKeys.includes(key)) {
        if (this.openKeys.length >= MAX_OPEN_PANELS) {
          return;
        }
        this.openKeys.push(key);
      }
      this.focusedKey = key;
    },

    // Hides the panel and fixes up focus. Never touches the underlying
    // session/process - shared by the soft-close and hard-close paths below
    // so neither has to duplicate it (and so they never call each other).
    _removeFromOpen(key) {
      this.openKeys = this.openKeys.filter((k) => k !== key);
      if (this.focusedKey === key) {
        this.focusedKey = this.openKeys.at(-1) ?? null;
      }
    },

    // The panel's only close affordance. If a turn is actively running,
    // preserve it - just hide the panel, exactly like before. If it's idle,
    // also stop (and dispose) it outright: with only 4 concurrent slots and
    // no separate manual "stop" button anymore, leaving an idle session
    // parked here would waste a slot the user has no other way to reclaim -
    // closing one panel to open another must not leave a ghost process
    // behind. `stopSession()` is a safe no-op if the session was never even
    // connected (e.g. the user only opened the panel to look at the tiling
    // layout and never sent a message).
    async closePanel(key) {
      const store = useZeroSessionStore(key);
      if (store.runInProgress) {
        this._removeFromOpen(key);
        return;
      }
      await store.stopSession();
      delete this.keyMeta[key];
      store.$dispose();
      this._removeFromOpen(key);
    },

    focusPanel(key) {
      if (this.openKeys.includes(key)) {
        this.focusedKey = key;
      }
    },

    // Unconditional stop, regardless of whether a turn is running - used
    // when the user deletes the underlying session entirely (see
    // MainLayout's onDeleteSession), never from the panel's own close
    // button.
    async stopAndDispose(key) {
      const store = useZeroSessionStore(key);
      await store.stopSession();
      delete this.keyMeta[key];
      store.$dispose();
      this._removeFromOpen(key);
    },
  },
});

export async function openOrFocusSession(key, cwd, sessionId) {
  const runtime = useSessionRuntimeStore();
  if (!runtime.canOpenMore && !runtime.isOpen(key)) {
    return { error: "SESSION_CAP_REACHED" };
  }
  runtime.openPanel(key);
  const store = useZeroSessionStore(key);
  if (!store.cwd) {
    // Only prepare the panel (load history if resuming a persisted
    // session) - the real zero acp process isn't spawned until the user
    // actually sends a message (see sendMessage's own lazy-connect in
    // zero-session-store.js). This is what lets someone open/close panels
    // to try out the tiling layout without silently spinning up processes
    // that would occupy one of the 4 concurrent slots for nothing.
    await store.prepareSession(cwd, sessionId);
  }
  return { ok: true };
}

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useSessionRuntimeStore, import.meta.hot));
}
