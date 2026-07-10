import { defineStore, acceptHMRUpdate } from "pinia";

const STORAGE_KEY = "zero-desktop-workspaces";

function loadWorkspaces() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed;
    }
  } catch {
    // ignore corrupt data
  }
  return [];
}

function saveWorkspaces(workspaces) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(workspaces));
}

export const useWorkspacesStore = defineStore("workspaces", {
  state: () => ({
    workspaces: loadWorkspaces(),
    activePath: null,
  }),

  getters: {
    active: (state) => state.workspaces.find((w) => w.path === state.activePath) || null,
    hasActive: (state) => state.activePath !== null,
  },

  actions: {
    add(path) {
      const normalized = path.replace(/\/+$/, "") || "/";
      const name = normalized.split("/").pop() || normalized;

      if (this.workspaces.some((w) => w.path === normalized)) return;

      this.workspaces.push({
        path: normalized,
        name,
        addedAt: Date.now(),
      });
      saveWorkspaces(this.workspaces);

      if (!this.activePath) {
        this.activePath = normalized;
      }
    },

    remove(path) {
      this.workspaces = this.workspaces.filter((w) => w.path !== path);
      saveWorkspaces(this.workspaces);
      if (this.activePath === path) {
        this.activePath = this.workspaces.length > 0 ? this.workspaces[0].path : null;
      }
    },

    select(path) {
      this.activePath = path;
    },

    reorder(fromIndex, toIndex) {
      const item = this.workspaces.splice(fromIndex, 1)[0];
      this.workspaces.splice(toIndex, 0, item);
      saveWorkspaces(this.workspaces);
    },
  },
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useWorkspacesStore, import.meta.hot));
}
