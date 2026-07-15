import { defineStore, acceptHMRUpdate } from "pinia";
import { useZeroSessionStore } from "@/stores/zero-session-store";

export const MAX_OPEN_PANELS = 4;

export const useSessionRuntimeStore = defineStore("session-runtime", {
  state: () => ({
    // Flat, all workspaces together. The backend imposes no global cap on
    // concurrent zero acp processes anymore - the user may run as many as
    // they want. The 4-panel cap is now PER WORKSPACE: each workspace can
    // have up to MAX_OPEN_PANELS open panels, and panels belonging to other
    // workspaces keep running in the background without interfering.
    openKeys: [],
    // Per-workspace, so switching workspaces doesn't leave focus pointing
    // at a now-hidden key from the workspace you left, and coming back
    // restores exactly which panel had focus there.
    focusedKeyByPath: {},
    keyMeta: {},
  }),

  getters: {
    // Count non-placeholder panels for a given workspace. Blank placeholders
    // never spawn a process (they only call prepareSession(cwd, null), which
    // defers the real connection until sendMessage), so they must NOT count
    // against the per-workspace cap.
    panelCountFor: (state) => (workspacePath) =>
      state.openKeys.filter(
        (k) => state.keyMeta[k]?.cwd === workspacePath && !state.keyMeta[k]?.isBlankPlaceholder,
      ).length,
    canOpenMore: (state) => (workspacePath) =>
      state.openKeys.filter(
        (k) => state.keyMeta[k]?.cwd === workspacePath && !state.keyMeta[k]?.isBlankPlaceholder,
      ).length < MAX_OPEN_PANELS,
    isOpen: (state) => (key) => state.openKeys.includes(key),
    // The panels a given workspace should actually render in its tiling
    // grid - every other open key (belonging to a different workspace)
    // keeps running/tracked in the background, just not shown here.
    visibleKeys: (state) => (workspacePath) =>
      state.openKeys.filter((k) => state.keyMeta[k]?.cwd === workspacePath),
    focusedKeyFor: (state) => (workspacePath) => state.focusedKeyByPath[workspacePath] ?? null,
  },

  actions: {
    registerMeta(key, patch) {
      this.keyMeta[key] = { ...this.keyMeta[key], ...patch };
    },

    openPanel(key, workspacePath) {
      if (!this.openKeys.includes(key)) {
        // Per-workspace cap: each workspace can have up to MAX_OPEN_PANELS
        // real (non-placeholder) panels. Panels from other workspaces don't
        // count against this workspace's limit.
        if (this.panelCountFor(workspacePath) >= MAX_OPEN_PANELS) {
          return;
        }
        // If the only panel currently visible here is the blank
        // placeholder closePanel() auto-creates so the workspace is never
        // left without one, replace it instead of stacking a second panel
        // next to it - it was never a session the user deliberately opened
        // to keep around, so opening something real should just become the
        // one panel, not add to it. Without this, resuming a session right
        // after closing your last panel silently opened it as an unfocused
        // second panel behind the still-empty placeholder, looking exactly
        // like "recovering a session does nothing."
        const visible = this.visibleKeys(workspacePath);
        if (
          visible.length === 1 &&
          visible[0] !== key &&
          this.keyMeta[visible[0]]?.isBlankPlaceholder
        ) {
          const placeholderKey = visible[0];
          this.openKeys = this.openKeys.filter((k) => k !== placeholderKey);
          delete this.keyMeta[placeholderKey];
          const placeholderStore = useZeroSessionStore(placeholderKey);
          placeholderStore.stopSession().finally(() => {
            // $dispose() only unregisters the store instance - it leaves the
            // underlying state object cached in pinia by id, which a future
            // store for the same key would silently inherit stale (see the
            // two disposals below). This key is a fresh UUID that will never
            // be reused, so it's not load-bearing here, but reset-then-
            // dispose is the only combination that's actually safe.
            placeholderStore.$reset();
            placeholderStore.$dispose();
            // stopSession()'s own finally block calls _syncRuntimeMeta(),
            // which resurrects a keyMeta entry for this key after the delete
            // above (it runs later, once the stop actually resolves) - delete
            // it again now that stopSession has truly finished so this
            // disposed placeholder doesn't linger in keyMeta forever.
            delete this.keyMeta[placeholderKey];
          });
        }
        this.openKeys.push(key);
      }
      this.focusedKeyByPath[workspacePath] = key;
    },

    // Hides the panel and fixes up focus for whichever workspace it
    // belonged to. Never touches the underlying session/process - shared
    // by the soft-close and hard-close paths below so neither has to
    // duplicate it (and so they never call each other).
    _removeFromOpen(key) {
      const workspacePath = this.keyMeta[key]?.cwd;
      this.openKeys = this.openKeys.filter((k) => k !== key);
      if (workspacePath && this.focusedKeyByPath[workspacePath] === key) {
        const remaining = this.visibleKeys(workspacePath);
        this.focusedKeyByPath[workspacePath] = remaining.at(-1) ?? null;
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
    async closePanel(key, { replaceIfLast = true } = {}) {
      const workspacePath = this.keyMeta[key]?.cwd;
      const store = useZeroSessionStore(key);
      if (store.runInProgress) {
        this._removeFromOpen(key);
      } else {
        await store.stopSession();
        delete this.keyMeta[key];
        // $dispose() alone leaves this key's state cached in pinia (it only
        // unregisters the store instance, not the raw state object) - a
        // later reopen of this same key (e.g. resuming this exact session)
        // would otherwise silently inherit the stale cwd/sessionId and skip
        // re-preparing entirely. Reset before dispose so the next store
        // built for this key starts from real defaults.
        store.$reset();
        store.$dispose();
        this._removeFromOpen(key);
      }
      // Se o usuário fechou o último painel visível do workspace ativo,
      // abre um novo painel vazio para que a área de trabalho nunca fique
      // sem painel. Não fazemos isso quando o fechamento veio de estouro
      // de capacidade, para evitar loop.
      if (replaceIfLast && workspacePath && this.visibleKeys(workspacePath).length === 0) {
        const { openOrFocusSession } = await import("@/stores/session-runtime-store");
        const newKey = crypto.randomUUID();
        await openOrFocusSession(newKey, workspacePath, null);
        // Tagged AFTER opening (registerMeta merges, so this survives the
        // _syncRuntimeMeta patch prepareSession already applied) - marks it
        // as disposable filler so openPanel replaces it instead of stacking
        // a second panel the next time something real gets opened here.
        this.registerMeta(newKey, { isBlankPlaceholder: true });
      }
    },

    focusPanel(key, workspacePath) {
      if (this.openKeys.includes(key)) {
        this.focusedKeyByPath[workspacePath] = key;
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
      store.$reset();
      store.$dispose();
      this._removeFromOpen(key);
    },
  },
});

export async function openOrFocusSession(key, cwd, sessionId) {
  const runtime = useSessionRuntimeStore();
  if (!runtime.canOpenMore(cwd) && !runtime.isOpen(key)) {
    return { error: "SESSION_CAP_REACHED" };
  }
  runtime.openPanel(key, cwd);
  const store = useZeroSessionStore(key);
  const isResuming = sessionId && store.sessionId !== sessionId;
  if (!store.cwd || isResuming) {
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
