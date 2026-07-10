import { defineStore, acceptHMRUpdate } from 'pinia'
import {
  locateZeroCli,
  startZeroSession,
  sendZeroMessage,
  stopZeroSession,
  onZeroEvent,
  onZeroStderr,
  listZeroSessions,
  loadSessionHistory,
  deleteSession,
} from '@/services/zero'

export const useZeroStore = defineStore('zero', {
  state: () => ({
    zeroPath: null,
    zeroVersion: null,
    zeroError: null,
    isConnected: false,
    isConnecting: false,
    messages: [],
    currentResponse: '',
    currentWorkspace: '',
    currentSessionId: null,
    sessions: [],
    unlistenEvent: null,
    unlistenStderr: null,
  }),

  getters: {
    hasZero: (state) => Boolean(state.zeroPath),
  },

  actions: {
    async locateZero() {
      this.zeroError = null
      try {
        const location = await locateZeroCli()
        this.zeroPath = location.path
        this.zeroVersion = location.version
      } catch (error) {
        this.zeroPath = null
        this.zeroVersion = null
        this.zeroError = error
      }
    },

    async startSession(cwd, sessionId = null) {
      if (!cwd) {
        this.zeroError = 'No workspace provided'
        return
      }

      if (this.isConnected || this.isConnecting) {
        await this.stopSession()
      }

      this.messages = []
      this.currentResponse = ''
      this.currentWorkspace = cwd
      this.currentSessionId = sessionId
      this.isConnecting = true
      this.zeroError = null

      try {
        await this.setupListeners()
        await startZeroSession(cwd, sessionId)
        this.isConnected = true
      } catch (error) {
        this.zeroError = error
        this.isConnected = false
      } finally {
        this.isConnecting = false
      }
    },

    async sendMessage(content) {
      if (!this.isConnected) {
        this.zeroError = 'Not connected to a workspace'
        return
      }

      this.addUserMessage(content)
      this.currentResponse = ''

      try {
        await sendZeroMessage(content)
      } catch (error) {
        this.zeroError = error
      }
    },

    async stopSession() {
      try {
        await stopZeroSession()
      } finally {
        this.isConnected = false
        this.currentResponse = ''
        this.removeListeners()
      }
    },

    async loadSessions(cwd) {
      try {
        this.sessions = await listZeroSessions(cwd)
      } catch {
        this.sessions = []
      }
    },

    async openSession(sessionId) {
      if (!this.currentWorkspace) return
      this.currentSessionId = sessionId
      this.messages = []
      this.currentResponse = ''

      try {
        const history = await loadSessionHistory(sessionId)
        this.messages = history.map((msg) => ({
          role: msg.role,
          content: msg.content,
          timestamp: new Date(msg.timestamp).getTime(),
        }))
      } catch {
        this.messages = []
      }
    },

    async removeSession(sessionId) {
      try {
        console.log('[zero-store] deleteSession:', sessionId)
        await deleteSession(sessionId)
        if (this.currentSessionId === sessionId) {
          this.currentSessionId = null
          this.messages = []
        }
        if (this.currentWorkspace) {
          await this.loadSessions(this.currentWorkspace)
        }
      } catch (err) {
        console.error('[zero-store] deleteSession failed:', err)
        this.zeroError = String(err)
      }
    },

    async setupListeners() {
      this.removeListeners()

      this.unlistenEvent = await onZeroEvent((event) => {
        this.handleZeroEvent(event.payload)
      })

      this.unlistenStderr = await onZeroStderr((event) => {
        console.log('[zero stderr]', event.payload)
      })
    },

    removeListeners() {
      if (this.unlistenEvent) {
        this.unlistenEvent()
        this.unlistenEvent = null
      }
      if (this.unlistenStderr) {
        this.unlistenStderr()
        this.unlistenStderr = null
      }
    },

    handleZeroEvent(event) {
      switch (event.type) {
        case 'run_start':
          this.currentResponse = ''
          break
        case 'text':
          this.currentResponse += event.delta || ''
          break
        case 'final':
          this.addAssistantMessage(event.text || this.currentResponse)
          this.currentResponse = ''
          break
        case 'run_end':
          if (this.currentResponse) {
            this.addAssistantMessage(this.currentResponse)
            this.currentResponse = ''
          }
          if (this.currentWorkspace) {
            this.loadSessions(this.currentWorkspace)
          }
          break
        case 'error':
          this.zeroError = event.message
          break
        case 'tool_call':
        case 'tool_result':
        case 'permission_request':
          this.addEventMessage(event)
          break
        default:
          // Ignore unknown events for now
          break
      }
    },

    addUserMessage(content) {
      this.messages.push({ role: 'user', content, timestamp: Date.now() })
    },

    addAssistantMessage(content) {
      this.messages.push({ role: 'assistant', content, timestamp: Date.now() })
    },

    addSystemMessage(content) {
      this.messages.push({ role: 'system', content, timestamp: Date.now() })
    },

    addEventMessage(event) {
      this.messages.push({
        role: 'event',
        content: `[${event.type}] ${JSON.stringify(event)}`,
        timestamp: Date.now(),
      })
    },
  },
})

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useZeroStore, import.meta.hot))
}
