import { defineStore, acceptHMRUpdate } from "pinia";
import { i18n } from "@/i18n/instance";
import {
  locateZeroCli,
  startZeroSession,
  sendZeroMessage,
  stopZeroSession,
  onZeroEvent,
  onZeroStderr,
  onZeroProcessExited,
  listZeroSessions,
  loadSessionHistory,
  deleteSession,
  sendPermissionDecision,
} from "@/services/zero";

const MAX_STDERR_LINES = 20;
const SESSION_SYNC_INTERVAL_MS = 3000;

let _idCounter = 0;
function nextId() {
  return `msg-${++_idCounter}`;
}

export const useZeroStore = defineStore("zero", {
  state: () => ({
    zeroPath: null,
    zeroVersion: null,
    zeroError: null,
    isConnected: false,
    isConnecting: false,
    messages: [],
    currentResponse: "",
    currentThinking: "",
    currentWorkspace: "",
    currentSessionId: null,
    sessions: [],
    unlistenEvent: null,
    unlistenStderr: null,
    unlistenProcessExited: null,
    runInProgress: false,
    isLoadingSession: false,
    lastStderrLines: [],
    currentPlan: [],
    _sessionSyncTimer: null,
    _lastEventCount: 0,
  }),

  getters: {
    hasZero: (state) => Boolean(state.zeroPath),

    workingStatus(state) {
      if (state.currentThinking) return "thinking";
      const runningTool = state.messages.find(
        (m) => m.type === "tool_call" && m.status === "running",
      );
      if (runningTool) return { type: "tool", toolName: runningTool.toolName };
      if (state.currentResponse) return "writing";
      if (state.runInProgress) return "sending";
      return null;
    },

    // update_plan calls replace the whole plan each time, so the latest one
    // is the current state of the todo list. Hide it once every item is
    // checked off instead of leaving a stale "all done" list pinned above
    // the input.
    activePlan(state) {
      if (!state.currentPlan || state.currentPlan.length === 0) return null;
      const allDone = state.currentPlan.every((item) => item.status === "completed");
      return allDone ? null : state.currentPlan;
    },
  },

  actions: {
    async locateZero() {
      this.zeroError = null;
      try {
        const location = await locateZeroCli();
        this.zeroPath = location.path;
        this.zeroVersion = location.version;
      } catch (error) {
        this.zeroPath = null;
        this.zeroVersion = null;
        this.zeroError = error;
      }
    },

    async startSession(cwd, sessionId = null) {
      if (!cwd) {
        this.zeroError = "No workspace provided";
        return;
      }

      if (this.isConnected || this.isConnecting) {
        await this.stopSession();
      }

      this.messages = [];
      this.currentResponse = "";
      this.currentThinking = "";
      this.currentWorkspace = cwd;
      this.currentSessionId = sessionId;
      this.isConnecting = true;
      this.zeroError = null;
      this.runInProgress = false;
      this.lastStderrLines = [];
      this.currentPlan = [];
      this._lastEventCount = 0;

      try {
        await this.setupListeners();
        await startZeroSession(cwd, sessionId);
        this.isConnected = true;
        this._startSessionSync();
      } catch (error) {
        this.zeroError = error;
        this.isConnected = false;
      } finally {
        this.isConnecting = false;
      }
    },

    async sendMessage(content) {
      if (!this.isConnected) {
        this.zeroError = "Not connected to a workspace";
        return;
      }

      this.addUserMessage(content);
      this.currentResponse = "";
      this.currentThinking = "";
      this.runInProgress = true;

      try {
        await sendZeroMessage(content);
      } catch (error) {
        this.zeroError = error;
        this.runInProgress = false;
      }
    },

    async stopSession() {
      try {
        await stopZeroSession();
      } finally {
        this.isConnected = false;
        this.currentResponse = "";
        this.currentThinking = "";
        this.runInProgress = false;
        this.removeListeners();
        this._stopSessionSync();
      }
    },

    async loadSessions(cwd) {
      try {
        this.sessions = await listZeroSessions(cwd);
      } catch {
        this.sessions = [];
      }
    },

    async openSession(sessionId) {
      if (!this.currentWorkspace) return;
      this.currentSessionId = sessionId;
      this.messages = [];
      this.currentResponse = "";
      this.currentThinking = "";
      this.currentPlan = [];
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

      this._startSessionSync();
    },

    _startSessionSync() {
      this._stopSessionSync();
      if (!this.currentSessionId) return;

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
      if (!this.currentSessionId || this.runInProgress || this.isConnecting) return;

      try {
        const events = await loadSessionHistory(this.currentSessionId);
        if (events.length === this._lastEventCount) return;

        this._lastEventCount = events.length;
        this.buildMessagesFromHistory(events);
      } catch {
        // Ignore sync errors to avoid spamming the user while a session is
        // temporarily unavailable.
      }
    },

    // Persisted session events (events.jsonl) use different field names than
    // the live stream for the same concepts (tool_call: `arguments` JSON
    // string + `id` vs. live `args` object + `id`; tool_result: `toolCallId`
    // vs. live `id`). This adapts each persisted event into the shape the
    // existing add*/update* helpers already expect, so restoring a session
    // renders thinking/tool-call/permission cards the same way a live run
    // does instead of only showing plain text messages.
    buildMessagesFromHistory(events) {
      this.messages = [];
      // Load permission decisions saved from a previous live session so that
      // recovered permission cards show the correct approved/denied status
      // instead of all appearing as "pending" with fresh Approve/Deny buttons.
      const savedDecisions = this.currentSessionId
        ? this._loadPermissionDecisions(this.currentSessionId)
        : {};

      for (const event of events) {
        const payload = event.payload || {};
        switch (event.type) {
          case "message":
            if (!payload.content) break;
            if (payload.role === "user") {
              this.addUserMessage(payload.content);
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
            let args = {};
            try {
              args =
                typeof payload.arguments === "string"
                  ? JSON.parse(payload.arguments)
                  : payload.arguments || {};
            } catch {
              args = {};
            }
            this.addToolCall({ name: payload.name, id: payload.id, args });
            break;
          }

          case "tool_result":
            this.updateToolCallResult({
              id: payload.toolCallId,
              status: payload.status,
              output: payload.output,
            });
            break;

          case "permission_request": {
            this.addPermissionRequest(payload);
            // Restore any decision the user made during the live session.
            const permId = payload.toolCallId || payload.permissionId;
            if (permId && savedDecisions[permId]) {
              const msg = this.messages[this.messages.length - 1];
              if (msg && msg.type === "permission_request") {
                msg.status = savedDecisions[permId];
              }
            }
            break;
          }

          case "permission_decision": {
            // When a permission_decision event exists in history, try to
            // update the matching permission_request card instead of creating
            // a separate badge so the card reflects the final state.
            const permId = payload.permissionId;
            const action = payload.action;
            const req = permId
              ? this.messages.find(
                  (m) => m.type === "permission_request" && m.permissionId === permId,
                )
              : null;
            if (req) {
              req.status = action === "deny" ? "denied" : "approved";
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

    // ---- localStorage helpers for permission decisions --------------------
    // Because send_permission_decision cannot deliver decisions mid-run
    // (zero reads stdin to EOF before acting), decisions are persisted
    // client-side so that recovered sessions show the correct
    // approved/denied state instead of resetting every card to "pending".

    _permStorageKey(sessionId) {
      return `zero-desktop-permissions-${sessionId}`;
    },

    _loadPermissionDecisions(sessionId) {
      try {
        const raw = localStorage.getItem(this._permStorageKey(sessionId));
        return raw ? JSON.parse(raw) : {};
      } catch {
        return {};
      }
    },

    _savePermissionDecision(sessionId, permissionId, decision) {
      const decisions = this._loadPermissionDecisions(sessionId);
      decisions[permissionId] = decision;
      try {
        localStorage.setItem(this._permStorageKey(sessionId), JSON.stringify(decisions));
      } catch {
        // localStorage full or unavailable — non-critical.
      }
    },

    _clearPermissionDecisions(sessionId) {
      try {
        localStorage.removeItem(this._permStorageKey(sessionId));
      } catch {
        // ignore
      }
    },

    async removeSession(sessionId) {
      try {
        await deleteSession(sessionId);
        this._clearPermissionDecisions(sessionId);
        if (this.currentSessionId === sessionId) {
          this.currentSessionId = null;
          this.messages = [];
          this._stopSessionSync();
        }
        if (this.currentWorkspace) {
          await this.loadSessions(this.currentWorkspace);
        }
      } catch (err) {
        this.zeroError = String(err);
      }
    },

    async setupListeners() {
      this.removeListeners();

      this.unlistenEvent = await onZeroEvent((event) => {
        this.handleZeroEvent(event.payload);
      });

      this.unlistenStderr = await onZeroStderr((event) => {
        this.lastStderrLines.push(event.payload);
        if (this.lastStderrLines.length > MAX_STDERR_LINES) {
          this.lastStderrLines.shift();
        }
      });

      this.unlistenProcessExited = await onZeroProcessExited(() => {
        this.handleProcessExited();
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
    },

    handleProcessExited() {
      if (!this.runInProgress) return;

      this.finalizeThinking();
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

      this.currentResponse = "";
      this.currentThinking = "";
      this.runInProgress = false;
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

        case "permission_request":
          this.finalizeThinking();
          this.addPermissionRequest(event);
          break;

        case "permission_decision":
          this.finalizeThinking();
          this.addPermissionDecision(event);
          break;

        case "run_end":
          this.finalizeThinking();
          if (this.currentResponse) {
            this.addAssistantMessage(this.currentResponse);
            this.currentResponse = "";
          }
          this.runInProgress = false;
          if (this.currentWorkspace) {
            this.loadSessions(this.currentWorkspace);
          }
          break;

        case "error":
          this.finalizeThinking();
          this.zeroError = event.message;
          this.runInProgress = false;
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

    addUserMessage(content) {
      this.messages.push({
        id: nextId(),
        type: "text",
        role: "user",
        content,
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

      // update_plan replaces the whole plan each call - track the latest one
      // separately so it can be pinned above the input (see activePlan).
      // Reused for both live events and history replay since both funnel
      // through this same method.
      if (event.name === "update_plan" && Array.isArray(event.args?.plan)) {
        this.currentPlan = event.args.plan;
      }
    },

    updateToolCallResult(event) {
      const toolUseId = event.id;
      const msg = this.messages.find(
        (m) => m.type === "tool_call" && m.toolUseId === toolUseId && m.status === "running",
      );
      if (msg) {
        msg.status = event.status === "error" ? "error" : "completed";
        msg.result = event.output || "";
      }
    },

    addPermissionRequest(event) {
      const args = event.args || {};
      const commandSummary =
        typeof args.cmd === "string"
          ? args.cmd
          : Object.keys(args).length
            ? JSON.stringify(args)
            : "";
      this.messages.push({
        id: nextId(),
        type: "permission_request",
        permissionId: event.toolCallId || event.permissionId,
        toolName: event.name || event.toolName,
        proposedCommand: commandSummary || event.proposedCommand || "",
        reason: event.reason || event.justification || "",
        status: "pending",
        timestamp: Date.now(),
      });
    },

    addPermissionDecision(event) {
      this.messages.push({
        id: nextId(),
        type: "permission_decision",
        toolName: event.name,
        action: event.action,
        reason: event.reason || "",
        riskLevel: event.risk?.level || "",
        timestamp: Date.now(),
      });
    },

    async approvePermission(permissionId) {
      const msg = this.messages.find(
        (m) =>
          m.type === "permission_request" &&
          m.permissionId === permissionId &&
          m.status === "pending",
      );
      if (!msg) return;
      msg.status = "approved";
      if (this.currentSessionId) {
        this._savePermissionDecision(this.currentSessionId, permissionId, "approved");
      }
      try {
        await sendPermissionDecision(permissionId, "approved");
      } catch (error) {
        this.zeroError = error;
      }
    },

    async denyPermission(permissionId) {
      const msg = this.messages.find(
        (m) =>
          m.type === "permission_request" &&
          m.permissionId === permissionId &&
          m.status === "pending",
      );
      if (!msg) return;
      msg.status = "denied";
      if (this.currentSessionId) {
        this._savePermissionDecision(this.currentSessionId, permissionId, "denied");
      }
      try {
        await sendPermissionDecision(permissionId, "denied");
      } catch (error) {
        this.zeroError = error;
      }
    },
  },
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useZeroStore, import.meta.hot));
}
