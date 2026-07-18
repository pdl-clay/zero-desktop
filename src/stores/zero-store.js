import { defineStore, acceptHMRUpdate } from "pinia";
import {
  locateZeroCli,
  listZeroModels,
  listMcpBackends,
  checkMcpBackend,
  listMcpTools,
  loadMcpStatusCache,
  listZeroProviderCatalog,
  listZeroProviders,
  addZeroProvider,
  removeZeroProvider,
  useZeroProvider,
  checkZeroProvider,
  getAdvisorConfig,
  setAdvisorConfig,
} from "@/services/zero";

export const useZeroStore = defineStore("zero", {
  state: () => ({
    zeroPath: null,
    zeroVersion: null,
    zeroError: null,
    availableModels: [],
    // Additive: { [modelId]: { reasoning: boolean, reasoningEfforts: string[] } }.
    // availableModels itself stays a plain string[] - see loadAvailableModels.
    modelCapabilities: {},
    activeModel: null,
    isLoadingModels: false,
    _modelsLoaded: false,
    mcpBackends: [],
    mcpTools: [],
    isLoadingMcp: false,
    _mcpLoaded: false,
    providerCatalog: [],
    configuredProviders: [],
    isLoadingProviders: false,
    _catalogLoaded: false,
    // The global advisor default (Settings dialog "General" tab) - the
    // fallback a session's advisor model/mode adopt when the user hasn't
    // made an explicit per-session choice yet. Cached here (rather than
    // fetched ad hoc from each session store) so every panel sees the same
    // value immediately after it's changed in Settings, without waiting for
    // a reload. See zero-session-store.js's toggleAdvisor.
    defaultAdvisorConfig: { enabled: false, model: null, mode: "max" },
    _advisorDefaultLoaded: false,
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
        const { models, active, capabilities } = await listZeroModels();
        this.availableModels = models;
        this.modelCapabilities = capabilities || {};
        this.activeModel = active;
        this._modelsLoaded = true;
      } catch (error) {
        this.zeroError = error;
      } finally {
        this.isLoadingModels = false;
      }
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

    // Catalog is static per zero CLI version, so it's cached like MCP
    // backends/models above - reloaded only when the Settings dialog forces
    // it (e.g. on open).
    async loadProviderCatalog({ force = false } = {}) {
      if (this._catalogLoaded && !force) return;
      try {
        this.providerCatalog = await listZeroProviderCatalog();
        this._catalogLoaded = true;
      } catch {
        this.providerCatalog = [];
      }
    },

    // Configured providers can change any time (add/remove/use), so unlike
    // the catalog this is always meant to be refetched with force after a
    // mutation rather than trusted from a stale cache.
    async loadConfiguredProviders({ force = false } = {}) {
      if (this.configuredProviders.length > 0 && !force) return;
      this.isLoadingProviders = true;
      try {
        this.configuredProviders = await listZeroProviders();
      } catch {
        this.configuredProviders = [];
      } finally {
        this.isLoadingProviders = false;
      }
    },

    // Adding a provider with setActive changes the active provider globally
    // (see providers.rs::add), so the available-models cache - which
    // resolves the active provider under the hood - must be refreshed too,
    // otherwise ChatInput's model picker keeps showing the old provider's
    // models until a session reconnects.
    async addProvider(req) {
      await addZeroProvider(req);
      await this.loadConfiguredProviders({ force: true });
      if (req.setActive) {
        await this.loadAvailableModels({ force: true });
      }
    },

    async removeProvider(name) {
      await removeZeroProvider(name);
      await this.loadConfiguredProviders({ force: true });
    },

    async useProvider(name) {
      await useZeroProvider(name);
      await this.loadConfiguredProviders({ force: true });
      await this.loadAvailableModels({ force: true });
    },

    async loadDefaultAdvisorConfig({ force = false } = {}) {
      if (this._advisorDefaultLoaded && !force) return this.defaultAdvisorConfig;
      try {
        this.defaultAdvisorConfig = await getAdvisorConfig();
        this._advisorDefaultLoaded = true;
      } catch {
        // Keep whatever was cached (or the disabled default) if it can't be loaded.
      }
      return this.defaultAdvisorConfig;
    },

    async saveDefaultAdvisorConfig(config) {
      this.defaultAdvisorConfig = config;
      this._advisorDefaultLoaded = true;
      await setAdvisorConfig(config);
    },

    async checkProvider(name, { connectivity = false } = {}) {
      const provider = this.configuredProviders.find((p) => p.name === name);
      if (provider) {
        provider._checking = true;
        provider._error = null;
      }
      try {
        const result = await checkZeroProvider(name, connectivity);
        if (provider) {
          provider.status = result.status;
          provider._health = result.health || null;
        }
      } catch (err) {
        if (provider) {
          provider.status = "error";
          provider._error = String(err);
        }
      } finally {
        if (provider) {
          provider._checking = false;
        }
      }
    },
  },
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useZeroStore, import.meta.hot));
}
