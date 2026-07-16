<template>
  <div class="file-explorer-tree">
    <div v-if="nodes.length === 0 && !loading" class="file-explorer-tree__empty">
      {{ $t("explorer.empty") }}
    </div>
    <q-spinner-dots v-else-if="loading" size="24px" color="primary" class="q-ma-md" />
    <q-tree v-else :nodes="nodes" node-key="key" dense @lazy-load="onLazyLoad">
      <template #default-header="prop">
        <div
          class="file-explorer-tree__node row items-center no-wrap"
          :draggable="prop.node.isFile"
          @dragstart="prop.node.isFile && onDragStart($event, prop.node)"
        >
          <q-icon :name="prop.node.icon" size="16px" class="q-mr-xs" />
          <span class="ellipsis">{{ prop.node.label }}</span>
        </div>
      </template>
    </q-tree>
  </div>
</template>

<script setup>
import { ref, watch } from "vue";
import { useQuasar } from "quasar";
import { useI18n } from "vue-i18n";
import { listDirectoryEntries } from "@/services/zero";
import { getFileIcon } from "@/utils/file";
import { attachFileToPanel } from "@/utils/attach-file-to-panel";
import { useWorkspacesStore } from "@/stores/workspaces-store";
import { useSessionRuntimeStore } from "@/stores/session-runtime-store";

const props = defineProps({
  rootPath: {
    type: String,
    default: null,
  },
});

const $q = useQuasar();
const { t } = useI18n();
const workspacesStore = useWorkspacesStore();
const sessionRuntime = useSessionRuntimeStore();

const nodes = ref([]);
const loading = ref(false);

function buildNode(entry) {
  if (entry.isDir) {
    return {
      label: entry.name,
      key: entry.path,
      icon: "folder",
      lazy: true,
      isFile: false,
    };
  }
  return {
    label: entry.name,
    key: entry.path,
    icon: getFileIcon(null, entry.name),
    isFile: true,
    handler: onFileClick,
  };
}

async function loadRoot(path) {
  if (!path) {
    nodes.value = [];
    return;
  }
  loading.value = true;
  try {
    const entries = await listDirectoryEntries(path);
    nodes.value = entries.map(buildNode);
  } catch (error) {
    nodes.value = [];
    $q.notify({ type: "negative", message: String(error), position: "top" });
  } finally {
    loading.value = false;
  }
}

watch(() => props.rootPath, loadRoot, { immediate: true });

async function onLazyLoad({ node, done, fail }) {
  try {
    const entries = await listDirectoryEntries(node.key);
    done(entries.map(buildNode));
  } catch (error) {
    $q.notify({ type: "negative", message: String(error), position: "top" });
    fail();
  }
}

// Click cites to whichever chat panel currently has focus - same
// resolution TerminalPanel.vue uses for its own "cite to chat" action.
async function onFileClick(node) {
  const focusedKey = sessionRuntime.focusedKeyFor(workspacesStore.activePath);
  if (!focusedKey) {
    $q.notify({ type: "warning", message: t("explorer.citeNoTarget"), position: "top" });
    return;
  }
  try {
    await attachFileToPanel(node.key, focusedKey);
  } catch (error) {
    $q.notify({ type: "negative", message: String(error), position: "top" });
  }
}

// Drag-and-drop is the other half of the hybrid citation flow: dropping a
// file onto a specific pane (see SessionTileGrid.vue's onDropFile) targets
// that exact panel regardless of focus - the path travels as plain text via
// a custom MIME type so the drop handler doesn't need any shared state.
function onDragStart(event, node) {
  event.dataTransfer.setData("application/x-zero-file", node.key);
  event.dataTransfer.setData("text/plain", node.key);
  event.dataTransfer.effectAllowed = "copy";
}
</script>

<style scoped>
.file-explorer-tree {
  height: 100%;
  overflow-y: auto;
}

.file-explorer-tree__empty {
  padding: 12px;
  font-size: 0.8em;
  color: var(--chat-text-muted);
}

.file-explorer-tree__node {
  cursor: pointer;
  min-width: 0;
}
</style>
