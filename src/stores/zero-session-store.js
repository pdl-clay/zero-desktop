import { defineStore, acceptHMRUpdate } from "pinia";
import { i18n } from "@/i18n/instance";
import {
  startZeroSession,
  sendZeroMessage,
  stopZeroSession,
  cancelZeroRun,
  onZeroEvent,
  onZeroStderr,
  onZeroProcessExited,
  onZeroPermissionRequest,
  loadSessionHistory,
  deleteSession,
  renameSession,
  respondToPermission as respondToPermissionApi,
  switchZeroModel,
  setSessionModelById,
  switchZeroEffort,
  setSessionEffortById,
  getSessionAdvisorConfig,
  setSessionAdvisorConfig,
  setSessionMode,
  setSessionModeById,
  getSessionPlanState,
  clearPendingSpec,
  readSpecFile,
} from "@/services/zero";
import { useSessionRuntimeStore } from "@/stores/session-runtime-store";
import { useZeroStore } from "@/stores/zero-store";
import { useWorkspacesStore } from "@/stores/workspaces-store";
import { isEditTool } from "@/utils/edit-tools";
import { isAdvisorConsultation, extractAdvisorPrompt } from "@/utils/advisor-prompt";

const MAX_STDERR_LINES = 20;
const SESSION_SYNC_INTERVAL_MS = 3000;

let _idCounter = 0;
function nextId() {
  return `msg-${++_idCounter}`;
}

// Advisor model + trigger mode are remembered across restarts via
// localStorage - separate from the enabled on/off state (a new chat still
// starts with advisor off by default, same as before), but once the user
// turns it on, the model/mode pickers should already reflect their last
// choice instead of coming up blank/default every time the app restarts.
// Backend session config (see _loadAdvisorConfig) can't carry this alone for
// a panel that hasn't connected yet (deferred until the first sendMessage,
// see startSession) - there's no session for the backend to have an opinion
// about. Until the user has explicitly picked a model/mode at least once
// (see hasExplicitAdvisorPreference), toggleAdvisor adopts the Settings
// dialog's global default (zeroStore.defaultAdvisorConfig) instead, so the
// settings popup doesn't come up blank the first time advisor is turned on.
const ADVISOR_PREFS_STORAGE_KEY = "zero-advisor-preferences";

function loadAdvisorPreferences() {
  try {
    const raw = localStorage.getItem(ADVISOR_PREFS_STORAGE_KEY);
    if (!raw) return { model: null, mode: "max" };
    const parsed = JSON.parse(raw);
    return {
      model: typeof parsed.model === "string" ? parsed.model : null,
      mode: parsed.mode === "low" ? "low" : "max",
    };
  } catch {
    return { model: null, mode: "max" };
  }
}

function saveAdvisorPreferences({ model, mode }) {
  try {
    localStorage.setItem(ADVISOR_PREFS_STORAGE_KEY, JSON.stringify({ model, mode }));
  } catch {
    // Best-effort - localStorage can throw in private-browsing-like
    // contexts or when full; losing the "remember my last choice"
    // convenience isn't worth surfacing an error to the user over.
  }
}

// Whether the user has ever explicitly picked an advisor model/mode (via the
// picker in ChatInput's settings popup or the Settings dialog's own model
// picker, both of which call saveAdvisorPreferences). Distinguishes "no
// opinion yet" from "explicitly chose the default" so toggleAdvisor below
// knows when it's still safe to adopt the Settings dialog's global default.
function hasExplicitAdvisorPreference() {
  try {
    return localStorage.getItem(ADVISOR_PREFS_STORAGE_KEY) !== null;
  } catch {
    return false;
  }
}

// Same rationale as ADVISOR_PREFS_STORAGE_KEY above: there's no "last used
// reasoning effort" concept on the backend (a genuinely new session always
// starts at "" / auto), so a freshly created panel's effort selector should
// still reflect the user's last choice instead of always starting at auto.
const REASONING_EFFORT_STORAGE_KEY = "zero-reasoning-effort-preference";

function loadReasoningEffortPreference() {
  try {
    return localStorage.getItem(REASONING_EFFORT_STORAGE_KEY) || "";
  } catch {
    return "";
  }
}

function saveReasoningEffortPreference(effort) {
  try {
    localStorage.setItem(REASONING_EFFORT_STORAGE_KEY, effort);
  } catch {
    // Best-effort, same rationale as saveAdvisorPreferences.
  }
}

export function useZeroSessionStore(key) {
  return defineStore(`zero-session:${key}`, {
    state: () => ({
      sessionKey: key,
      sessionId: null,
      cwd: null,
      // The chat compose box's text. Lives here (not a local ref in
      // ChatView.vue) so other parts of the app - e.g. the terminal panel's
      // "cite to chat" action - can insert text into whichever panel is
      // focused without reaching into another component's internals.
      draftText: "",
      // The compose box's pending attachment (one at a time): either a file
      // the user picked via ChatInput's attach button, or a citation the
      // terminal panel dropped in via "cite to chat" - both are the same
      // `{ mimeType, data, name }` shape `send_zero_message` already accepts,
      // rendered as the same chip in ChatInput.vue either way. Lives here for
      // the same reason draftText does: so the terminal panel can set it on
      // whichever panel is focused without reaching into ChatInput.vue.
      pendingAttachment: null,
      // The model THIS session's own process is actually running under -
      // snapshotted whenever it (re)connects, and updated only when THIS
      // session explicitly switches. Deliberately separate from the global
      // store's activeModel (see switchModel below): each panel's process
      // keeps whatever model it started with until it personally restarts,
      // so the picker must reflect that per panel, not one shared value.
      activeModel: null,
      // This session's reasoning-effort preference ("" = auto). Seeded from
      // the user's last choice (see loadReasoningEffortPreference) rather
      // than always starting at auto - mirrors advisorModel/advisorMode
      // above. Unlike activeModel, there's no meaningful "global default" to
      // realign against before sending: reasoning effort is purely a
      // per-session ACP override (_zero/set_effort), not mirrored into any
      // shared CLI config file.
      reasoningEffort: loadReasoningEffortPreference(),
      // True when switchEffort was called before this session had a
      // sessionId at all yet - mirrors _sessionModeDirty. Also starts true
      // whenever a non-auto preference was seeded from localStorage above:
      // unlike advisorModel/advisorMode (inert until advisorEnabled is
      // explicitly turned on), reasoningEffort IS the live value the picker
      // shows, so a seeded-but-never-pushed preference would otherwise leave
      // the UI showing a tier the backend never actually received. Flushed
      // (and cleared) by _flushReasoningEffortIfDirty once startSession gives
      // this panel a real id.
      _reasoningEffortDirty: loadReasoningEffortPreference() !== "",
      isConnected: false,
      isConnecting: false,
      messages: [],
      currentResponse: "",
      currentThinking: "",
      currentPlan: [],
      unlistenEvent: null,
      unlistenStderr: null,
      unlistenProcessExited: null,
      unlistenPermissionRequest: null,
      runInProgress: false,
      isLoadingSession: false,
      lastStderrLines: [],
      _cancelledByUser: false,
      _sessionSyncTimer: null,
      _lastEventCount: 0,
      // Advisor mode state. model/mode are seeded from localStorage (see
      // loadAdvisorPreferences) so a freshly created panel's settings popup
      // already shows the user's last choice instead of coming up blank -
      // enabled itself deliberately stays false here, so a new chat still
      // starts with advisor off by default.
      advisorEnabled: false,
      advisorModel: loadAdvisorPreferences().model,
      advisorMode: loadAdvisorPreferences().mode,
      // True when toggleAdvisor/setAdvisorModel/setAdvisorMode was called
      // before this session had a live backend process (see
      // prepareSession - the real connection is deferred to the first
      // sendMessage). Lets _loadAdvisorConfig know to push this panel's
      // locally-chosen config once startSession connects, instead of
      // overwriting it with whatever the freshly-registered backend
      // session defaulted to.
      _advisorConfigDirty: false,
      // This session's live ACP permission mode: "auto" (run safe tools
      // automatically, ask before risky ones), "ask" (ask before every tool
      // that changes state), or "spec-draft" (Plan Mode - read-only
      // exploration ending in a proposed implementation spec for review, the
      // native equivalent of Claude Code's Plan Mode). All three are driven
      // by the real ACP `session/set_mode`, enforced engine-side. Mirrors
      // what the Rust bridge persists per session_id
      // (session-plan-state.json) and re-applies across a crash/respawn -
      // see setMode/_syncPlanStateFromDisk.
      sessionMode: "auto",
      // The spec awaiting the user's decision: { specId, title, filePath,
      // relativePath, content }, or null. Populated either live (from a
      // spec_review_required event) or restored from disk on reconnect.
      pendingPlanReview: null,
      // True when setMode was called before this session had a sessionId at
      // all yet (brand-new, never-sent-a-message panel) - the only case with
      // nowhere to persist the choice yet. Flushed by _syncPlanStateFromDisk
      // once startSession gives this panel a real id.
      _sessionModeDirty: false,
    }),

    getters: {
      workingStatus(state) {
        if (state.currentThinking) return "thinking";
        const runningTool = (state.messages || []).find(
          (m) =>
            (m.type === "tool_call" || m.type === "advisor_consultation") && m.status === "running",
        );
        if (runningTool) {
          return {
            type: "tool",
            toolName:
              runningTool.type === "advisor_consultation" ? "advisor" : runningTool.toolName,
          };
        }
        if (state.currentResponse) return "writing";
        if (state.runInProgress) return "sending";
        return null;
      },

      activePlan(state) {
        if (!state.currentPlan || state.currentPlan.length === 0) return null;
        const allDone = state.currentPlan.every((item) => item.status === "completed");
        return allDone ? null : state.currentPlan;
      },

      editedFiles(state) {
        const order = [];
        const byPath = new Map();
        for (const m of state.messages || []) {
          if (m.type !== "tool_call") continue;
          if (!isEditTool(m.toolName)) continue;
          const path = m.input?.path;
          if (!path) continue;
          if (!byPath.has(path)) {
            byPath.set(path, { path, edits: [] });
            order.push(path);
          }
          byPath.get(path).edits.push(m);
        }
        return order.map((p) => byPath.get(p));
      },
    },

    actions: {
      _syncRuntimeMeta() {
        const runtime = useSessionRuntimeStore();
        const status = this.workingStatus;
        const pending = this.messages.find(
          (m) => m.type === "permission_request" && m.status === "pending" && m.answerable,
        );
        runtime.registerMeta(this.sessionKey, {
          cwd: this.cwd,
          sessionId: this.sessionId,
          title: this.messages.find((m) => m.type === "text" && m.role === "user")?.content || null,
          workingStatus: status,
          hasPendingPermission: Boolean(pending),
        });
      },

      // Sets up the panel's display state (cwd, and history if resuming a
      // persisted session) WITHOUT spawning the real zero acp process. Lets
      // a panel sit open - e.g. while the user is just trying out the
      // tiling layout - without occupying one of the 4 concurrent process
      // slots. The actual connection only happens lazily, the first time
      // sendMessage() is called (see there).
      async prepareSession(cwd, sessionId = null) {
        this.cwd = cwd;
        if (sessionId) {
          await this.openSession(sessionId);
        } else {
          this.sessionId = null;
          this.messages = [];
          this.currentResponse = "";
          this.currentThinking = "";
          this.currentPlan = [];
          this.sessionMode = "auto";
          this.pendingPlanReview = null;
        }
        this._syncRuntimeMeta();
      },

      async startSession(cwd, sessionId = null) {
        if (!cwd) {
          const globalStore = useZeroStore();
          globalStore.zeroError = "No workspace provided";
          return;
        }

        // If this is the same session prepareSession() already loaded
        // history for (the common case: sendMessage's lazy-connect
        // graduating an already-open, already-populated panel into a real
        // connection), keep the loaded transcript instead of wiping it -
        // resuming a session should never blank out what's already on
        // screen right as the user tries to continue it.
        const alreadyPrepared = Boolean(sessionId) && this.sessionId === sessionId;
        if (!alreadyPrepared) {
          this.messages = [];
          this.currentResponse = "";
          this.currentThinking = "";
          this.currentPlan = [];
          // Do NOT stomp sessionMode when a mode choice is still dirty (the
          // user picked e.g. Plan Mode via the dropdown on a brand-new panel
          // that had no sessionId yet - setMode had nowhere to persist to
          // but this in-memory field, see setMode/_sessionModeDirty). This
          // is the exact path sendMessage's lazy-connect takes for a first
          // message: without this guard, this reset ran AFTER setMode but
          // BEFORE _syncPlanStateFromDisk flushed the dirty choice, so it
          // silently overwrote "spec-draft" back to "auto" right before
          // pushing that (wrong) value to the engine - the mode picker
          // would visibly flip back to Auto and the model never received
          // the read-only/plan-drafting system prompt.
          if (!this._sessionModeDirty) {
            this.sessionMode = "auto";
          }
          this.pendingPlanReview = null;
          this._lastEventCount = 0;
        }
        this.cwd = cwd;
        this.sessionId = sessionId;
        this.isConnecting = true;
        this.zeroError = null;
        this.runInProgress = false;
        this.lastStderrLines = [];

        // No id was known before this call - this is a genuinely brand-new
        // session (the common case: onNewSession's blank panel, lazily
        // connected on its first sendMessage). The backend registers it in
        // `zero sessions list` the moment session/new succeeds, but nothing
        // else re-fetches the sidebar afterward - without this it only
        // showed up once the user left the workspace and came back.
        const isNewSession = !sessionId;

        try {
          await this.setupListeners();
          const result = await startZeroSession(this.sessionKey, cwd, sessionId);
          this.sessionId = result.sessionId;
          this.isConnected = true;
          if (isNewSession) {
            useWorkspacesStore().loadSessions(cwd);
          }
          // A freshly (re)spawned process picks up whatever the CLI's
          // global model config currently is - snapshot it here so this
          // panel's picker reflects what it's actually running, even if
          // another panel changes the global default afterwards.
          if (!this.activeModel) {
            const globalStore = useZeroStore();
            this.activeModel = globalStore.activeModel;
          }
          this._startSessionSync();
          // Load advisor config for this session
          await this._loadAdvisorConfig();
          // Restore (or push a pre-connection choice for) Plan Mode
          await this._syncPlanStateFromDisk();
          // Push a reasoning-effort choice made before this session connected
          await this._flushReasoningEffortIfDirty();
        } catch (error) {
          const globalStore = useZeroStore();
          const errorMsg = String(error);
          // The backend no longer enforces a global process cap, so
          // SESSION_CAP_REACHED should never arrive here. Keep the handler
          // as a defensive no-op in case an old backend is still running.
          if (errorMsg.startsWith("SESSION_CAP_REACHED")) {
            const runtime = useSessionRuntimeStore();
            runtime.closePanel(this.sessionKey, { replaceIfLast: false });
          }
          globalStore.zeroError = error;
          this.isConnected = false;
        } finally {
          this.isConnecting = false;
        }
        this._syncRuntimeMeta();
      },

      async sendMessage(content, file = null) {
        if (!this.cwd) {
          const globalStore = useZeroStore();
          globalStore.zeroError = "No workspace provided";
          return;
        }

        // Once actually used, this panel is a real session the user is
        // having a conversation in, not just the auto-created filler that
        // keeps the workspace from ever being left with zero panels -
        // opening a different session from now on must add a new panel
        // next to this one, not silently replace it (see openPanel).
        const runtime = useSessionRuntimeStore();
        if (runtime.keyMeta[this.sessionKey]?.isBlankPlaceholder) {
          runtime.registerMeta(this.sessionKey, { isBlankPlaceholder: false });
        }

        if (!this.isConnected) {
          await this.startSession(this.cwd, this.sessionId);
        }

        if (!this.isConnected) {
          return;
        }

        // The CLI's active model lives in a single global config file, read
        // live by every running process on every turn - verified empirically
        // (switching the model in one panel silently changed another
        // already-running panel's very next answer too, with no restart
        // involved). There's no per-process override available from the
        // CLI, so this is the best available mitigation: realign the global
        // config to what THIS panel believes its own model is, right before
        // it actually sends, so at least the answer it's about to get
        // matches what its own picker shows. A residual race remains if two
        // panels send in the exact same instant.
        await this._realignModelBeforeSend();

        this.addUserMessage(content, file);
        this.currentResponse = "";
        this.currentThinking = "";
        this.runInProgress = true;
        this._syncRuntimeMeta();

        try {
          await sendZeroMessage(this.sessionKey, content, file);
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
          this.runInProgress = false;
          this._syncRuntimeMeta();
        }
      },

      // See sendMessage above for why this exists. Only acts if this
      // session has an opinion (activeModel set) that has drifted from the
      // global config - a brand-new not-yet-connected session has neither,
      // so it just inherits whatever's currently active, same as before.
      async _realignModelBeforeSend() {
        const globalStore = useZeroStore();
        if (!this.activeModel || this.activeModel === globalStore.activeModel) return;
        try {
          await switchZeroModel(this.sessionKey, this.activeModel);
          globalStore.activeModel = this.activeModel;
        } catch (error) {
          globalStore.zeroError = error;
        }
      },

      async cancelRun() {
        if (!this.runInProgress) return;
        this._cancelledByUser = true;
        try {
          await cancelZeroRun(this.sessionKey);
        } catch (error) {
          this._cancelledByUser = false;
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
      },

      // Switches the model for THIS session only - the CLI setting itself
      // is global (there's no per-session model in the underlying
      // protocol), but only this session's process actually gets
      // cancelled/restarted to pick it up. Every other open panel keeps
      // running under whatever model it already snapshotted, so
      // `this.activeModel` (not the global one) is what the picker must
      // compare against and display for THIS panel.
      async switchModel(model) {
        if (model === this.activeModel || this.runInProgress) return;
        this.activeModel = model;
        // Also updates the global "default" so any panel that hasn't
        // connected yet (and will snapshot from it on first connect)
        // picks up the new choice going forward.
        const globalStore = useZeroStore();
        globalStore.activeModel = model;
        try {
          if (this.isConnected) {
            await switchZeroModel(this.sessionKey, model);
          } else if (this.sessionId) {
            // A session exists (e.g. reopened from history) but hasn't
            // (re)connected yet - switch_zero_model requires a live
            // `sessions` entry keyed by this panel, which only exists once
            // connected at least once; calling it here throws "No active
            // session for key: ...". Persist by session id instead - picked
            // up automatically on next connect (see the model-reapply block
            // in spawn_and_handshake). If the user sends a message before
            // that, _realignModelBeforeSend covers the gap too.
            await setSessionModelById(this.sessionId, model);
          }
          // Else: a brand-new panel with no sessionId yet - nothing to
          // persist to. _realignModelBeforeSend (called from sendMessage,
          // right after startSession connects) pushes this.activeModel once
          // a real session exists.
        } catch (error) {
          globalStore.zeroError = error;
        }
      },

      // Switches this session's reasoning-effort preference. Three-way
      // branch mirrors switchModel exactly, minus the global-config-realign
      // concern: effort has no shared CLI config file to keep in sync with,
      // just this session's own ACP-side override.
      async switchEffort(effort) {
        if (effort === this.reasoningEffort || this.runInProgress) return;
        this.reasoningEffort = effort;
        saveReasoningEffortPreference(effort);
        try {
          if (this.isConnected) {
            await switchZeroEffort(this.sessionKey, effort);
          } else if (this.sessionId) {
            await setSessionEffortById(this.sessionId, effort);
          } else {
            this._reasoningEffortDirty = true;
          }
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
      },

      // Flushes a reasoningEffort choice made before this session had a
      // sessionId yet (see switchEffort/_reasoningEffortDirty) - called from
      // startSession right after connecting, mirroring
      // _syncPlanStateFromDisk's _sessionModeDirty flush.
      async _flushReasoningEffortIfDirty() {
        if (!this._reasoningEffortDirty || !this.sessionId) return;
        this._reasoningEffortDirty = false;
        try {
          await setSessionEffortById(this.sessionId, this.reasoningEffort);
        } catch {
          // best-effort, same rationale as _syncPlanStateFromDisk's flush
        }
      },

      /**
       * Toggle advisor mode for this session.
       * @param {boolean} enabled
       */
      async toggleAdvisor(enabled) {
        // The user hasn't explicitly picked an advisor model/mode for any
        // panel yet (loadAdvisorPreferences seeded model/mode from nothing),
        // so turning advisor on here should adopt the Settings dialog's
        // global default instead of coming up with model=null/mode="max" -
        // see hasExplicitAdvisorPreference and defaultAdvisorConfig.
        if (enabled && !hasExplicitAdvisorPreference()) {
          const globalStore = useZeroStore();
          const defaults = await globalStore.loadDefaultAdvisorConfig();
          this.advisorModel = defaults.model ?? this.advisorModel;
          this.advisorMode = defaults.mode || this.advisorMode;
        }
        this.advisorEnabled = enabled;
        // A panel can sit open without a live backend process (see
        // prepareSession - the real connection is deferred to the first
        // sendMessage), so there's no session yet for the backend to
        // attach this config to. Trying anyway throws "No active session
        // for key: ...". Mark it dirty instead; startSession pushes the
        // locally-chosen config once it actually connects (see
        // _loadAdvisorConfig).
        if (!this.isConnected) {
          this._advisorConfigDirty = true;
          return;
        }
        try {
          await setSessionAdvisorConfig(this.sessionKey, {
            enabled,
            model: this.advisorModel,
            mode: this.advisorMode,
          });
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
      },

      /**
       * Set the advisor model for this session.
       * @param {string | null} model
       */
      async setAdvisorModel(model) {
        this.advisorModel = model;
        saveAdvisorPreferences({ model, mode: this.advisorMode });
        if (!this.isConnected) {
          this._advisorConfigDirty = true;
          return;
        }
        try {
          await setSessionAdvisorConfig(this.sessionKey, {
            enabled: this.advisorEnabled,
            model,
            mode: this.advisorMode,
          });
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
      },

      /**
       * Set the advisor trigger mode ("max" = proactive/broad, "low" =
       * restrictive, StepFun-style) for this session.
       * @param {"max" | "low"} mode
       */
      async setAdvisorMode(mode) {
        this.advisorMode = mode;
        saveAdvisorPreferences({ model: this.advisorModel, mode });
        if (!this.isConnected) {
          this._advisorConfigDirty = true;
          return;
        }
        try {
          await setSessionAdvisorConfig(this.sessionKey, {
            enabled: this.advisorEnabled,
            model: this.advisorModel,
            mode,
          });
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
      },

      /**
       * Syncs advisor config with the backend right after this session
       * connects. If the user toggled advisor before the session had a
       * live process (_advisorConfigDirty), that explicit local choice
       * wins - it's pushed to the backend. Otherwise, this panel hasn't
       * expressed an opinion yet, so it loads whatever the freshly
       * registered backend session defaulted to (the global config) - EXCEPT
       * model/mode, which stay whatever localStorage seeded them to: a
       * brand-new session's backend config is always the bare
       * {model: null, mode: "max"} default, and blindly adopting that would
       * clobber the user's remembered choice with nothing every time. Model
       * uses `??` since null is the backend's genuine "no opinion" value;
       * mode has no such sentinel (it's always "max" or "low"), so it's only
       * adopted when the backend config is actually enabled - i.e. a
       * resumed/already-configured session with a real opinion, not a fresh
       * default.
       */
      async _loadAdvisorConfig() {
        if (this._advisorConfigDirty) {
          this._advisorConfigDirty = false;
          try {
            await setSessionAdvisorConfig(this.sessionKey, {
              enabled: this.advisorEnabled,
              model: this.advisorModel,
              mode: this.advisorMode,
            });
          } catch {
            // Best-effort - the toggle already reflects the user's intent
            // locally; a failed push here just means the backend won't
            // honor it this turn.
          }
          return;
        }
        try {
          const config = await getSessionAdvisorConfig(this.sessionKey);
          this.advisorEnabled = config.enabled;
          this.advisorModel = config.model ?? this.advisorModel;
          if (config.enabled) {
            this.advisorMode = config.mode || this.advisorMode;
          }
        } catch {
          // Session might not have a config yet, use defaults
        }
      },

      /**
       * Toggle Plan Mode for this session (ACP "spec-draft" mode - read-only
       * exploration ending in a proposed implementation spec for review,
       * the native equivalent of Claude Code's Plan Mode). All three modes
       * ("auto" | "ask" | "spec-draft") are the real ACP `session/set_mode`,
       * enforced by the engine itself - not a client-side approximation.
       * Three cases, from most to least common: a connected session pushes
       * the change live; a known-but-disconnected session (e.g. reopened
       * from history, not yet resumed) persists it to disk directly, picked
       * up automatically on next connect; a brand-new panel with no
       * sessionId yet has nowhere to persist to, so the choice is just
       * marked dirty and flushed by _syncPlanStateFromDisk once
       * startSession connects.
       * @param {"auto" | "ask" | "spec-draft"} mode
       */
      async setMode(mode) {
        this.sessionMode = mode;
        try {
          if (this.isConnected) {
            await setSessionMode(this.sessionKey, mode);
          } else if (this.sessionId) {
            await setSessionModeById(this.sessionId, mode);
          } else {
            this._sessionModeDirty = true;
          }
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
      },

      /**
       * Restore sessionMode/pendingPlanReview from the backend's
       * disk-persisted session-plan-state.json, or flush a choice made
       * before this session had a sessionId (_sessionModeDirty). Called
       * after openSession (browsing history, no live connection needed) and
       * after startSession connects (covers reconnecting after an app
       * restart).
       */
      async _syncPlanStateFromDisk() {
        if (!this.sessionId) return;
        if (this._sessionModeDirty) {
          this._sessionModeDirty = false;
          try {
            await setSessionMode(this.sessionKey, this.sessionMode);
          } catch {
            // best-effort, same rationale as _loadAdvisorConfig's dirty flush
          }
          return;
        }
        try {
          const state = await getSessionPlanState(this.sessionId);
          this.sessionMode = state?.mode || "auto";
          if (state?.pendingSpec && !this.pendingPlanReview) {
            try {
              const content = await readSpecFile(state.pendingSpec.filePath);
              this.pendingPlanReview = { ...state.pendingSpec, content };
            } catch {
              // The spec file is gone from disk - self-heal the orphaned record.
              await clearPendingSpec(this.sessionId).catch(() => {});
            }
          }
        } catch {
          // keep current values
        }
      },

      /**
       * Fetch the submitted spec's markdown and open the review dialog.
       * Called from handleZeroEvent when a spec_review_required event
       * arrives live during this app run - the Rust bridge has already
       * persisted the pending spec as a side effect, this just fetches the
       * content to display now.
       */
      async _loadPlanReview(event) {
        try {
          const content = await readSpecFile(event.filePath);
          this.pendingPlanReview = { ...event, content };
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
      },

      /**
       * Approve the pending spec: flip the session's mode to the chosen
       * target ("auto" = implement automatically, "ask" = review each
       * edit), then send the implementation instruction as a normal
       * follow-up prompt in this same session (mirrors `zero spec approve`,
       * but as one continuous ACP conversation instead of forking a new
       * session - there is no store.RecordSpec on the ACP path, so the
       * CLI's `zero spec approve` cannot be used here).
       * @param {"auto" | "ask"} mode
       * @param {string} [comment]
       */
      async approvePlanReview(mode, comment = "") {
        const review = this.pendingPlanReview;
        if (!review) return;
        this.pendingPlanReview = null;
        await clearPendingSpec(this.sessionId).catch(() => {});
        this.sessionMode = mode;
        try {
          if (this.isConnected) {
            await setSessionMode(this.sessionKey, mode);
          } else if (this.sessionId) {
            await setSessionModeById(this.sessionId, mode);
          }
        } catch (error) {
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
        const base = i18n.global.t("chat.planReviewApproveInstruction", { title: review.title });
        const content = comment.trim() ? `${base}\n\n${comment.trim()}` : base;
        await this.sendMessage(content);
      },

      /**
       * Request changes: mode stays spec-draft (still read-only) - just
       * clear the persisted pending spec and send the feedback as the next
       * ordinary prompt; the agent revises and calls submit_spec again.
       * @param {string} feedback
       */
      async requestPlanChanges(feedback) {
        if (!feedback || !feedback.trim()) return;
        this.pendingPlanReview = null;
        await clearPendingSpec(this.sessionId).catch(() => {});
        await this.sendMessage(feedback.trim());
      },

      async stopSession() {
        try {
          await stopZeroSession(this.sessionKey);
        } finally {
          this.isConnected = false;
          this.currentResponse = "";
          this.currentThinking = "";
          this.runInProgress = false;
          this.removeListeners();
          this._stopSessionSync();
          this._syncRuntimeMeta();
        }
      },

      async openSession(sessionId) {
        this.sessionId = sessionId;
        this.messages = [];
        this.currentResponse = "";
        this.currentThinking = "";
        this.currentPlan = [];
        this.sessionMode = "auto";
        this.pendingPlanReview = null;
        this._lastEventCount = 0;
        this.isLoadingSession = true;

        try {
          const events = await loadSessionHistory(sessionId);
          this._lastEventCount = events.length;
          this.buildMessagesFromHistory(events);
        } catch {
          this.messages = [];
        } finally {
          this.isLoadingSession = false;
        }

        // Restore the Plan Mode toggle and an eventual pending plan review
        // for this session even before reconnecting a live process (session
        // recovery from history).
        await this._syncPlanStateFromDisk();

        this._startSessionSync();
        this._syncRuntimeMeta();
      },

      _startSessionSync() {
        this._stopSessionSync();
        if (!this.sessionId) return;

        this._sessionSyncTimer = setInterval(() => {
          this._syncSessionHistory();
        }, SESSION_SYNC_INTERVAL_MS);
      },

      _stopSessionSync() {
        if (this._sessionSyncTimer) {
          clearInterval(this._sessionSyncTimer);
          this._sessionSyncTimer = null;
        }
      },

      async _syncSessionHistory() {
        if (!this.sessionId || this.runInProgress || this.isConnecting) return;

        try {
          const events = await loadSessionHistory(this.sessionId);
          if (events.length === this._lastEventCount) return;

          this._lastEventCount = events.length;
          this.buildMessagesFromHistory(events);
        } catch {
          // Ignore sync errors to avoid spamming the user while a session is
          // temporarily unavailable.
        }
      },

      buildMessagesFromHistory(events) {
        this.messages = [];

        for (const event of events) {
          const payload = event.payload || {};
          switch (event.type) {
            case "message":
              if (!payload.content && !payload.file) break;
              if (payload.role === "user") {
                this.addUserMessage(payload.content || "", payload.file || null);
              } else {
                this.addAssistantMessage(payload.content);
              }
              break;

            case "reasoning":
              if (payload.content || payload.delta) {
                this.messages.push({
                  id: nextId(),
                  type: "thinking",
                  content: payload.content || payload.delta,
                  timestamp: Date.now(),
                });
              }
              break;

            case "tool_call": {
              let args = payload.args;
              if (args === undefined) {
                try {
                  args =
                    typeof payload.arguments === "string"
                      ? JSON.parse(payload.arguments)
                      : payload.arguments || {};
                } catch {
                  args = {};
                }
              }
              this.addToolCall({ name: payload.name, id: payload.id, args });
              break;
            }

            case "tool_result":
              this.updateToolCallResult({
                id: payload.id || payload.toolCallId,
                status: payload.status,
                output: payload.output,
              });
              break;

            case "plan_update":
              if (Array.isArray(payload.entries)) {
                this.currentPlan = payload.entries;
              }
              break;

            case "permission_request":
              this.addPermissionRequest({ ...payload, answerable: false });
              break;

            case "permission_decision": {
              const answered = this.messages.find(
                (m) => m.type === "permission_request" && m.requestId === payload.requestId,
              );
              if (answered) {
                answered.status = payload.action === "deny" ? "denied" : "approved";
              } else {
                this.addPermissionDecision(payload);
              }
              break;
            }

            case "error":
              this.messages.push({
                id: nextId(),
                type: "error",
                content: payload.message || "",
                timestamp: Date.now(),
              });
              break;

            default:
              break;
          }
        }
      },

      async removeSession(sessionId, onRefresh) {
        try {
          if (this.sessionId === sessionId) {
            await this.stopSession();
            this.sessionId = null;
            this.messages = [];
            this.currentPlan = [];
            this.sessionMode = "auto";
            this.pendingPlanReview = null;
          }
          await deleteSession(sessionId);
          if (onRefresh) await onRefresh();
        } catch (err) {
          const globalStore = useZeroStore();
          globalStore.zeroError = String(err);
        }
      },

      async renameSession(sessionId, title, onRefresh) {
        const trimmed = title.trim();
        if (!trimmed) return;
        try {
          await renameSession(sessionId, trimmed);
          if (onRefresh) await onRefresh();
        } catch (err) {
          const globalStore = useZeroStore();
          globalStore.zeroError = String(err);
        }
      },

      async setupListeners() {
        this.removeListeners();

        this.unlistenEvent = await onZeroEvent((event) => {
          if (event.payload?.sessionKey !== this.sessionKey) return;
          this.handleZeroEvent(event.payload);
        });

        this.unlistenStderr = await onZeroStderr((event) => {
          if (event.payload?.sessionKey !== this.sessionKey) return;
          this.lastStderrLines.push(event.payload?.line ?? "");
          if (this.lastStderrLines.length > MAX_STDERR_LINES) {
            this.lastStderrLines.shift();
          }
        });

        this.unlistenProcessExited = await onZeroProcessExited((event) => {
          if (event.payload?.sessionKey !== this.sessionKey) return;
          this.handleProcessExited();
        });

        this.unlistenPermissionRequest = await onZeroPermissionRequest((event) => {
          if (event.payload?.sessionKey !== this.sessionKey) return;
          console.log("[zero-session] permission request received:", event.payload);
          this.finalizeThinking();
          this.addPermissionRequest(event.payload);
          this._syncRuntimeMeta();
        });
      },

      removeListeners() {
        if (this.unlistenEvent) {
          this.unlistenEvent();
          this.unlistenEvent = null;
        }
        if (this.unlistenStderr) {
          this.unlistenStderr();
          this.unlistenStderr = null;
        }
        if (this.unlistenProcessExited) {
          this.unlistenProcessExited();
          this.unlistenProcessExited = null;
        }
        if (this.unlistenPermissionRequest) {
          this.unlistenPermissionRequest();
          this.unlistenPermissionRequest = null;
        }
      },

      handleProcessExited() {
        if (!this.runInProgress) return;

        this.finalizeThinking();

        if (this._cancelledByUser) {
          this._cancelledByUser = false;
        } else {
          const tail = this.lastStderrLines.slice(-5).join("\n");
          const content = tail
            ? `${i18n.global.t("chat.connectionLost")}\n${tail}`
            : i18n.global.t("chat.connectionLost");

          this.messages.push({
            id: nextId(),
            type: "error",
            content,
            timestamp: Date.now(),
          });
        }

        for (const msg of this.messages) {
          if (msg.status !== "running") continue;
          if (msg.type === "tool_call") {
            msg.status = "error";
            msg.result = i18n.global.t("chat.cancelled");
          } else if (msg.type === "advisor_consultation") {
            msg.status = "error";
            msg.content = i18n.global.t("chat.cancelled");
          }
        }

        this.currentResponse = "";
        this.currentThinking = "";
        this.currentPlan = [];
        // Deliberately NOT resetting sessionMode here: this is a crash of
        // the live process, not a session switch. The Rust bridge already
        // reapplies spec-draft automatically on the next respawn (see
        // spawn_and_handshake) - resetting to "auto" here would show it in
        // the UI while the engine is still read-only underneath.
        // pendingPlanReview is cleared locally (no live process to answer a
        // decision against) but comes back on its own once startSession
        // reconnects and runs _syncPlanStateFromDisk, since the persisted
        // record isn't touched here.
        this.pendingPlanReview = null;
        this.runInProgress = false;
        this._syncRuntimeMeta();
      },

      handleZeroEvent(event) {
        switch (event.type) {
          case "run_start":
            this.currentResponse = "";
            this.currentThinking = "";
            break;

          case "reasoning":
            this.currentThinking += event.delta || "";
            break;

          case "text":
            this.finalizeThinking();
            this.currentResponse += event.delta || "";
            break;

          case "final":
            this.finalizeThinking();
            this.addAssistantMessage(event.content || event.text || this.currentResponse);
            this.currentResponse = "";
            break;

          case "tool_call":
            this.finalizeThinking();
            this.addToolCall(event);
            break;

          case "tool_result":
            this.updateToolCallResult(event);
            break;

          case "permission_decision":
            this.finalizeThinking();
            this.addPermissionDecision(event);
            break;

          case "plan_update":
            if (Array.isArray(event.entries)) {
              this.currentPlan = event.entries;
            }
            break;

          case "spec_review_required":
            this.finalizeThinking();
            this._loadPlanReview(event);
            break;

          case "run_end":
            this.finalizeThinking();
            if (this.currentResponse) {
              this.addAssistantMessage(this.currentResponse);
              this.currentResponse = "";
            }
            this.runInProgress = false;
            this._syncRuntimeMeta();
            break;

          case "error":
            this.finalizeThinking();
            {
              const globalStore = useZeroStore();
              globalStore.zeroError = event.message;
            }
            this.runInProgress = false;
            this._syncRuntimeMeta();
            break;

          default:
            break;
        }
      },

      finalizeThinking() {
        if (!this.currentThinking) return;
        this.messages.push({
          id: nextId(),
          type: "thinking",
          content: this.currentThinking,
          timestamp: Date.now(),
        });
        this.currentThinking = "";
      },

      addUserMessage(content, file = null) {
        this.messages.push({
          id: nextId(),
          type: "text",
          role: "user",
          content,
          file,
          timestamp: Date.now(),
        });
      },

      addAssistantMessage(content) {
        this.messages.push({
          id: nextId(),
          type: "text",
          role: "assistant",
          content,
          timestamp: Date.now(),
        });
      },

      addSystemMessage(content) {
        this.messages.push({
          id: nextId(),
          type: "text",
          role: "system",
          content,
          timestamp: Date.now(),
        });
      },

      addToolCall(event) {
        if (event.name === "update_plan") {
          if (Array.isArray(event.args?.plan)) {
            this.currentPlan = event.args.plan;
          }
          return;
        }

        if (isAdvisorConsultation(event.name, event.args)) {
          this.messages.push({
            id: nextId(),
            type: "advisor_consultation",
            toolUseId: event.id,
            prompt: extractAdvisorPrompt(event.args) || "",
            content: "",
            status: "running",
            timestamp: Date.now(),
          });
          return;
        }

        this.messages.push({
          id: nextId(),
          type: "tool_call",
          toolName: event.name,
          toolUseId: event.id,
          input: event.args || {},
          status: "running",
          result: null,
          timestamp: Date.now(),
        });
      },

      updateToolCallResult(event) {
        const toolUseId = event.id;
        const msg = this.messages.find(
          (m) =>
            (m.type === "tool_call" || m.type === "advisor_consultation") &&
            m.toolUseId === toolUseId &&
            m.status === "running",
        );
        if (!msg) return;
        msg.status = event.status === "error" ? "error" : "completed";
        if (msg.type === "advisor_consultation") {
          msg.content = event.output || "";
        } else {
          msg.result = event.output || "";
        }
      },

      addPermissionRequest(event) {
        this.messages.push({
          id: nextId(),
          type: "permission_request",
          requestId: event.requestId || event.toolCallId || event.permissionId,
          toolName: event.toolName || event.name,
          reason: event.reason || event.justification || "",
          options: event.options || [],
          answerable: event.answerable !== false,
          status: "pending",
          timestamp: Date.now(),
        });
      },

      addPermissionDecision(event) {
        this.messages.push({
          id: nextId(),
          type: "permission_decision",
          toolName: event.name || event.toolName,
          action: event.action,
          reason: event.reason || "",
          riskLevel: event.risk?.level || "",
          timestamp: Date.now(),
        });
      },

      async respondToPermission(requestId, optionId) {
        console.log("[zero-session] respondToPermission called:", requestId, optionId);
        const msg = this.messages.find(
          (m) =>
            m.type === "permission_request" && m.requestId === requestId && m.status === "pending",
        );
        if (!msg) {
          console.warn("[zero-session] no pending permission message found for", requestId);
          return;
        }
        msg.status = optionId.startsWith("allow") ? "approved" : "denied";
        msg.chosenOptionId = optionId;
        try {
          await respondToPermissionApi(requestId, optionId);
          console.log("[zero-session] permission response sent:", requestId, optionId);
        } catch (error) {
          console.error("[zero-session] permission response failed:", error);
          const globalStore = useZeroStore();
          globalStore.zeroError = error;
        }
        this._syncRuntimeMeta();
      },
    },
  })();
}

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useZeroSessionStore, import.meta.hot));
}
