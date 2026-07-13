<template>
  <q-drawer
    v-model="drawerOpen"
    side="right"
    :width="320"
    :breakpoint="900"
    bordered
    overlay
    :class="$q.dark.isActive ? 'bg-dark' : 'bg-grey-2'"
    class="mcp-drawer"
  >
    <div class="column full-height">
      <!-- Header -->
      <div class="row items-center justify-between q-pa-md panel-header">
        <div class="text-subtitle1 text-weight-bold panel-header__title">
          {{ $t("mcp.title") }}
        </div>
        <div class="row items-center q-gutter-xs">
          <q-btn
            flat
            round
            dense
            size="sm"
            icon="refresh"
            color="grey-7"
            :loading="isLoading"
            @click="onRefreshAll"
          >
            <q-tooltip>{{ $t("mcp.refreshAll") }}</q-tooltip>
          </q-btn>
          <q-btn
            flat
            round
            dense
            size="sm"
            icon="close"
            color="grey-7"
            @click="drawerOpen = false"
          />
        </div>
      </div>

      <q-separator />

      <!-- Loading -->
      <div v-if="isLoading && backends.length === 0" class="col column items-center justify-center q-pa-xl">
        <q-spinner-dots size="40px" color="grey-7" />
        <div class="text-caption q-mt-sm panel-empty">{{ $t("mcp.loading") }}</div>
      </div>

      <!-- Empty state -->
      <div v-else-if="!isLoading && backends.length === 0" class="col column items-center justify-center q-pa-xl">
        <q-icon name="dns" size="48px" color="grey-6" />
        <div class="text-body2 q-mt-sm panel-empty">{{ $t("mcp.empty") }}</div>
        <div class="text-caption panel-empty">{{ $t("mcp.emptyHint") }}</div>
      </div>

      <!-- Server list -->
      <q-scroll-area v-else class="col" :visible="true">
        <q-list padding>
          <div
            v-for="backend in backends"
            :key="backend.name"
            class="mcp-item q-mb-xs"
          >
            <q-item
              clickable
              v-ripple
              class="rounded-borders"
              @click="onCheckBackend(backend.name)"
            >
              <q-item-section avatar>
                <q-icon
                  :name="backend.type === 'http' ? 'language' : 'terminal'"
                  :color="statusColor(backend)"
                  size="20px"
                />
              </q-item-section>

              <q-item-section>
                <q-item-label class="text-weight-medium mcp-item__name">
                  {{ backend.name }}
                </q-item-label>
                <q-item-label caption class="row items-center q-gutter-x-xs">
                  <q-badge
                    :color="backend.type === 'http' ? 'blue-2' : 'orange-2'"
                    :text-color="backend.type === 'http' ? 'blue-9' : 'orange-9'"
                    class="q-mr-xs"
                  >
                    {{ backend.type }}
                  </q-badge>
                  <span v-if="backend.url" class="mcp-item__url">
                    {{ truncateUrl(backend.url) }}
                  </span>
                  <span v-if="backend.status" class="mcp-item__status">
                    · {{ $t("mcp.toolCount", { count: backend.toolCount ?? 0 }) }}
                  </span>
                </q-item-label>
                <q-item-label
                  v-if="backend._error"
                  caption
                  class="text-negative"
                >
                  {{ backend._error }}
                </q-item-label>
              </q-item-section>

              <q-item-section side>
                <div class="row items-center q-gutter-x-xs">
                  <q-icon
                    v-if="backend._checking"
                    name="hourglass_empty"
                    size="16px"
                    color="grey-6"
                  />
                  <q-icon
                    v-else
                    :name="statusIcon(backend)"
                    :color="statusColor(backend)"
                    size="14px"
                  />
                  <q-icon
                    v-if="backend.status === 'ok'"
                    name="check_circle"
                    size="16px"
                    color="positive"
                  />
                  <q-icon
                    v-else-if="backend.status === 'error'"
                    name="error"
                    size="16px"
                    color="negative"
                  />
                </div>
              </q-item-section>
            </q-item>

            <!-- Expanded tools list -->
            <q-slide-transition>
              <div v-if="expandedName === backend.name && backend.tools && backend.tools.length > 0" class="q-px-md q-pb-sm">
                <div class="text-caption text-weight-medium q-mb-xs panel-section-label">
                  {{ $t("mcp.tools") }} ({{ backend.tools.length }})
                </div>
                <div
                  v-for="tool in backend.tools"
                  :key="tool.name"
                  class="mcp-tool-item row items-center q-px-sm q-py-xs rounded-borders q-mb-xs"
                >
                  <q-icon name="build" size="14px" color="grey-6" class="q-mr-sm" />
                  <span class="text-caption">{{ tool.name }}</span>
                </div>
              </div>
            </q-slide-transition>

            <!-- Expanded error -->
            <q-slide-transition>
              <div v-if="expandedName === backend.name && backend.status === 'error' && !(backend.tools && backend.tools.length > 0)" class="q-px-md q-pb-sm">
                <div class="text-caption text-negative">
                  {{ backend._error || $t("mcp.checkFailed") }}
                </div>
              </div>
            </q-slide-transition>
          </div>
        </q-list>
      </q-scroll-area>

      <!-- Info footer -->
      <div class="q-pa-sm text-center">
        <div class="text-caption panel-empty">
          {{ $t("mcp.footerHint") }}
        </div>
      </div>
    </div>
  </q-drawer>
</template>

<script setup>
import { ref } from "vue";
import { useZeroStore } from "@/stores/zero-store";
import { storeToRefs } from "pinia";

const zeroStore = useZeroStore();
const { mcpBackends: backends, isLoadingMcp: isLoading } = storeToRefs(zeroStore);

const drawerOpen = ref(false);
const expandedName = ref(null);

async function open() {
  drawerOpen.value = true;
  await zeroStore.loadMcpBackends({ force: true });
}

function close() {
  drawerOpen.value = false;
}

function toggle() {
  if (drawerOpen.value) {
    close();
  } else {
    open();
  }
}

async function onRefreshAll() {
  await zeroStore.loadMcpBackends({ force: true });
}

async function onCheckBackend(name) {
  if (expandedName.value === name) {
    expandedName.value = null;
    return;
  }

  await zeroStore.checkMcpBackend(name);
  expandedName.value = name;
}

function statusIcon(backend) {
  if (backend._checking) return "hourglass_empty";
  if (backend.status === "ok") return "cloud_done";
  if (backend.status === "error") return "cloud_off";
  return "cloud";
}

function statusColor(backend) {
  if (backend._checking) return "grey-6";
  if (backend.status === "ok") return "positive";
  if (backend.status === "error") return "negative";
  return "grey-6";
}

function truncateUrl(url) {
  if (!url) return "";
  try {
    const u = new URL(url);
    return u.hostname;
  } catch {
    return url.length > 28 ? url.slice(0, 28) + "…" : url;
  }
}

defineExpose({ open, close, toggle });
</script>

<style scoped>
.mcp-drawer {
  z-index: 2000;
}

.mcp-drawer :deep(.q-drawer) {
  z-index: 2000;
}

.panel-header {
  border-bottom: 1px solid var(--chat-card-border);
}

.panel-header__title {
  color: var(--chat-text);
}

.panel-section-label {
  color: var(--chat-text-muted);
}

.panel-empty {
  color: var(--chat-text-muted);
}

.mcp-item {
  border-radius: 10px;
}

.mcp-item :deep(.q-item) {
  border-radius: 10px;
  border: 1px solid transparent;
  color: var(--chat-text);
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.mcp-item :deep(.q-item:hover) {
  background: var(--chat-card-bg);
  border-color: var(--chat-card-border);
}

.mcp-item__name {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mcp-item__url {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: inline-block;
  color: var(--chat-text-muted);
}

.mcp-item__status {
  color: var(--chat-text-muted);
  white-space: nowrap;
}

.mcp-tool-item {
  background: var(--chat-card-bg);
  border: 1px solid var(--chat-card-border);
}
</style>
