import { defineStore, acceptHMRUpdate } from "pinia";
import {
  locateZeroCli,
  listZeroModels,
  listMcpBackends,
  checkMcpBackend,
  listMcpTools,
  loadMcpStatusCache,
} from "@/services/zero";

export const useZeroStore = defineStore("zero", {
  state: () => ({
    zeroPath: null,
    zeroVersion: null,
    zeroError: null,
    availableModels: [],
    activeModel: null,
    isLoadingModels: false,
    _modelsLoaded: false,
    permissionMode: "ask",
    mcpBackends: [],
    mcpTools: [],
    isLoadingMcp: false,
    _mcpLoaded: false,
  }),

  getters: {
    hasZero: (state) => Boolean(state.zeroPath),
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

    async loadAvailableModels({ force = false } = {}) {
      if (this._modelsLoaded && !force) return;
      this.isLoadingModels = true;
      try {
        const { models, active } = await listZeroModels();
        this.availableModels = models;
        this.activeModel = active;
        this._modelsLoaded = true;
      } catch (error) {
        this.zeroError = error;
      } finally {
        this.isLoadingModels = false;
      }
    },

    setPermissionMode(mode) {
      this.permissionMode = mode === "auto_allow" ? "auto_allow" : "ask";
    },

    async loadMcpBackends({ force = false } = {}) {
      if (this._mcpLoaded && !force) return;
      this.isLoadingMcp = true;
      try {
        const cache = await loadMcpStatusCache().catch(() => ({ servers: {} }));
        const [backends, tools] = await Promise.all([
          listMcpBackends(),
          listMcpTools().catch(() => []),
        ]);

        for (const backend of backends) {
          if (!backend.status) {
            const cached = cache.servers[backend.name];
            if (cached) {
              backend.status = cached.status;
              backend.error = cached.error || null;
              if (cached.toolCount > 0 && !backend.toolCount) {
                backend.toolCount = cached.toolCount;
              }
            }
          }
        }

        this.mcpBackends = backends;
        this.mcpTools = tools;
        this._mcpLoaded = true;

        if (this.mcpBackends.length > 0 && force) {
          this.checkAllMcpBackends();
        } else if (this.mcpBackends.length > 0) {
          const hasAnyCache = this.mcpBackends.some((b) => Boolean(b.status));
          if (!hasAnyCache) {
            this.checkAllMcpBackends();
          }
        }
      } catch {
        this.mcpBackends = [];
        this.mcpTools = [];
      } finally {
        this.isLoadingMcp = false;
      }
    },

    async loadMcpTools({ force = false } = {}) {
      if (this._mcpLoaded && !force) return;
      try {
        this.mcpTools = await listMcpTools();
      } catch {
        this.mcpTools = [];
      }
    },

    _toolsForBackend(backendName) {
      return this.mcpTools.filter((tool) =>
        tool.name.toLowerCase().startsWith(`${backendName.toLowerCase()}_`),
      );
    },

    _countToolsForBackend(backendName) {
      return this._toolsForBackend(backendName).length;
    },

    async checkAllMcpBackends() {
      await Promise.all(this.mcpBackends.map((backend) => this.checkMcpBackend(backend.name)));
    },

    async checkMcpBackend(name) {
      const backend = this.mcpBackends.find((b) => b.name === name);
      if (backend) {
        backend._checking = true;
      }
      try {
        const result = await checkMcpBackend(name);
        if (backend) {
          backend.status = result.status;
          if (result.tools && result.tools.length > 0) {
            backend.tools = result.tools.map((t) => (typeof t === "string" ? { name: t } : t));
            backend.toolCount = result.toolCount ?? backend.tools.length;
          } else {
            const tools = this._toolsForBackend(name);
            backend.tools = tools;
            backend.toolCount = result.toolCount ?? tools.length;
          }
          backend._error = result.error || null;
        }
      } catch (err) {
        if (backend) {
          backend.status = "error";
          backend._error = String(err);
        }
      } finally {
        if (backend) {
          backend._checking = false;
        }
      }
    },
  },
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useZeroStore, import.meta.hot));
}
