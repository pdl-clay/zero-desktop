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
    lastStderrLines: [],
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

      try {
        await this.setupListeners();
        await startZeroSession(cwd, sessionId);
        this.isConnected = true;
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

      try {
        const events = await loadSessionHistory(sessionId);
        this.buildMessagesFromHistory(events);
      } catch {
        this.messages = [];
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

          case "permission_request":
            this.addPermissionRequest(payload);
            break;

          case "permission_decision":
            this.addPermissionDecision(payload);
            break;

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

    async removeSession(sessionId) {
      try {
        console.log("[zero-store] deleteSession:", sessionId);
        await deleteSession(sessionId);
        if (this.currentSessionId === sessionId) {
          this.currentSessionId = null;
          this.messages = [];
        }
        if (this.currentWorkspace) {
          await this.loadSessions(this.currentWorkspace);
        }
      } catch (err) {
        console.error("[zero-store] deleteSession failed:", err);
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
