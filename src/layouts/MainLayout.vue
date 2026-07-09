<template>
  <q-layout view="lHh Lpr lFf">
    <q-drawer
      v-model="leftDrawerOpen"
      show-if-above
      :width="300"
      :breakpoint="700"
      bordered
      :class="$q.dark.isActive ? 'bg-dark' : 'bg-grey-2'"
    >
      <div class="row full-height">
        <!-- Left avatar column -->
        <div class="col-auto column items-center q-py-sm" style="width: 60px" :class="$q.dark.isActive ? 'bg-dark' : 'bg-grey-3'">
          <div class="q-pa-none">
            <img src="/assets/zero-logo.png" alt="Zero" class="drawer-logo" />
          </div>

          <q-separator class="full-width q-mb-sm" />

          <div class="workspace-column col">
            <div
              v-for="ws in workspacesStore.workspaces"
              :key="ws.path"
              class="workspace-avatar-wrapper q-mb-sm"
            >
              <div
                class="workspace-avatar cursor-pointer"
                :class="{ active: ws.path === workspacesStore.activePath }"
                :style="avatarStyle(ws)"
                @click="onSelectWorkspace(ws)"
              >
                {{ ws.name.charAt(0).toUpperCase() }}
              </div>
              <q-btn
                class="workspace-remove-btn"
                round
                dense
                flat
                size="xs"
                icon="close"
                color="negative"
                @click.stop="onRemoveWorkspace(ws)"
              >
                <q-tooltip>Remover {{ ws.name }}</q-tooltip>
              </q-btn>
              <q-tooltip anchor="center right" self="center left" :offset="[8, 0]">
                <div class="text-weight-bold">{{ ws.name }}</div>
                <div class="text-caption">{{ ws.path }}</div>
              </q-tooltip>
            </div>

            <div class="workspace-avatar-wrapper">
              <q-btn
                round
                flat
                size="sm"
                icon="add"
                color="grey-7"
                class="workspace-avatar"
                style="width:34px;height:34px"
                @click="onBrowseAndAdd"
              >
                <q-tooltip>{{ $t('workspace.add') }}</q-tooltip>
              </q-btn>
            </div>
          </div>

          <q-separator class="full-width q-mt-sm" />

          <q-btn
            flat
            round
            dense
            :icon="$q.dark.isActive ? 'light_mode' : 'dark_mode'"
            color="grey-7"
            size="sm"
            class="q-mt-xs"
            @click="toggleTheme"
          >
            <q-tooltip>{{ $q.dark.isActive ? 'Modo claro' : 'Modo escuro' }}</q-tooltip>
          </q-btn>

          <q-btn
            flat
            round
            dense
            icon="settings"
            color="grey-7"
            size="sm"
            class="q-mt-xs q-mb-xs"
          >
            <q-tooltip>Configurações</q-tooltip>
          </q-btn>

          
        </div>

        <!-- Right session panel -->
        <div class="col column q-pa-sm" style="min-width: 0">
          <!-- Zero status -->
          <q-banner
            v-if="!zeroStore.hasZero"
            class="bg-negative text-white q-mb-xs rounded-borders"
            dense
          >
            {{ $t('chat.zeroNotFound') }}
          </q-banner>

          <template v-if="workspacesStore.hasActive">
            <div class="text-caption text-grey-7 q-mb-xs">
              {{ workspacesStore.active?.name || '' }}
            </div>

            <q-separator spaced />

            <div class="text-caption text-grey-6 q-mb-sm">
              Sessões ({{ zeroStore.sessions.length }})
            </div>

            <q-scroll-area class="col">
              <!-- Session list -->
              <q-list dense>
                <div
                  v-for="session in zeroStore.sessions"
                  :key="session.session_id"
                  class="session-item-wrapper"
                >
                  <q-item
                    clickable
                    v-ripple
                    :active="session.session_id === zeroStore.currentSessionId"
                    active-class="bg-primary-1 text-primary"
                    class="rounded-borders q-px-sm"
                    @click="onSelectSession(session)"
                  >
                    <q-item-section side>
                      <q-icon :name="sessionIcon(session.kind)" size="16px" color="grey-6" />
                    </q-item-section>

                    <q-item-section>
                      <q-item-label class="text-body2" lines="1">
                        {{ session.title || session.session_id.slice(-8) }}
                      </q-item-label>
                      <q-item-label caption class="row items-center q-gutter-x-xs">
                        <span>{{ session.model_id }}</span>
                        <span>&middot;</span>
                        <span>{{ formatDate(session.created_at) }}</span>
                      </q-item-label>
                    </q-item-section>
                  </q-item>

                  <q-btn
                    class="session-remove-btn"
                    round
                    dense
                    flat
                    size="xs"
                    icon="close"
                    color="negative"
                    @click.stop="onDeleteSession(session)"
                  >
                    <q-tooltip>Excluir sessão</q-tooltip>
                  </q-btn>
                </div>

                <div
                  v-if="zeroStore.sessions.length === 0"
                  class="text-center text-grey-5 q-pa-md"
                >
                  <q-icon name="chat" size="28px" />
                  <div class="text-caption q-mt-xs">Nenhuma sessão neste workspace</div>
                </div>

                <q-item
                  clickable
                  v-ripple
                  class="rounded-borders q-px-sm text-grey-6"
                  @click="onNewSession"
                >
                  <q-item-section side>
                    <q-icon name="add_comment" size="16px" color="grey-5" />
                  </q-item-section>
                  <q-item-section>
                    <q-item-label class="text-body2">Nova sessão</q-item-label>
                  </q-item-section>
                </q-item>
              </q-list>
            </q-scroll-area>
          </template>

          <template v-else>
            <div class="flex flex-center col text-grey-5 text-center">
              <div>
                <q-icon name="folder_open" size="36px" />
                <div class="text-caption q-mt-sm">{{ $t('workspace.noWorkspaces') }}</div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </q-drawer>

    <!-- Main content -->
    <q-page-container>
      <q-page v-if="!workspacesStore.hasActive" class="flex flex-center">
        <div class="text-center text-grey-5">
          <q-icon name="chevron_left" size="60px" />
          <div class="text-h6 q-mt-md">{{ $t('workspace.select') }}</div>
        </div>
      </q-page>
      <ChatView v-else :workspace-path="workspacesStore.activePath" />
    </q-page-container>
  </q-layout>
</template>

<script setup>
import { ref, onMounted, watch } from 'vue'
import { useQuasar } from 'quasar'
import { useZeroStore } from '@/stores/zero-store'
import { useWorkspacesStore } from '@/stores/workspaces-store'
import { open } from '@tauri-apps/plugin-dialog'
import ChatView from '@/components/ChatView.vue'

const $q = useQuasar()
const zeroStore = useZeroStore()
const workspacesStore = useWorkspacesStore()
const leftDrawerOpen = ref(true)

const THEME_KEY = 'zero-desktop-theme'

const workspaceColors = [
  '#5c6bc0', '#26a69a', '#ffa726', '#ec407a', '#42a5f5',
  '#7e57c2', '#66bb6a', '#ef5350', '#8d6e63', '#26c6da',
]

function workspaceColor(name) {
  let hash = 0
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash)
  }
  return workspaceColors[Math.abs(hash) % workspaceColors.length]
}

function avatarStyle(ws) {
  const isActive = ws.path === workspacesStore.activePath
  const color = workspaceColor(ws.name)
  return {
    backgroundColor: color,
    width: isActive ? '40px' : '34px',
    height: isActive ? '40px' : '34px',
    fontSize: isActive ? '16px' : '12px',
    boxShadow: isActive ? `0 0 0 3px #fff, 0 0 0 5px ${color}` : 'none',
  }
}

function sessionIcon(kind) {
  switch (kind) {
    case 'fork': return 'call_split'
    case 'child': return 'subdirectory_arrow_right'
    default: return 'chat_bubble_outline'
  }
}

function formatDate(iso) {
  if (!iso) return ''
  const d = new Date(iso)
  return d.toLocaleDateString('pt-BR', { day: '2-digit', month: '2-digit', year: '2-digit' })
    + ' ' + d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' })
}

onMounted(async () => {
  const saved = localStorage.getItem(THEME_KEY)
  if (saved === 'dark') {
    $q.dark.set(true)
  }
  console.log('[MainLayout] workspaces loaded:', workspacesStore.workspaces.length, workspacesStore.workspaces)
  await zeroStore.locateZero()
})

function toggleTheme() {
  $q.dark.toggle()
  localStorage.setItem(THEME_KEY, $q.dark.isActive ? 'dark' : 'light')
}

watch(
  () => workspacesStore.activePath,
  async (newPath, oldPath) => {
    if (oldPath && zeroStore.isConnected) {
      await zeroStore.stopSession()
    }
    if (newPath && zeroStore.hasZero) {
      await zeroStore.startSession(newPath)
      await zeroStore.loadSessions(newPath)
    }
  },
)

async function onSelectWorkspace(ws) {
  workspacesStore.select(ws.path)
}

async function onSelectSession(session) {
  const cwd = workspacesStore.activePath
  if (!cwd) return

  await zeroStore.startSession(cwd, session.session_id)
  await zeroStore.openSession(session.session_id)
  await zeroStore.loadSessions(cwd)
}

async function onDeleteSession(session) {
  console.log('[MainLayout] onDeleteSession:', session.session_id, session)
  await zeroStore.removeSession(session.session_id)
}

async function onNewSession() {
  const cwd = workspacesStore.activePath
  if (!cwd) return
  await zeroStore.startSession(cwd)
}

async function onBrowseAndAdd() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: 'Selecionar diret\u00f3rio do projeto',
  })
  if (selected) {
    workspacesStore.add(selected)
  }
}

async function onRemoveWorkspace(ws) {
  workspacesStore.remove(ws.path)
}
</script>

<style scoped>
.drawer-logo {
  width: 32px;
  height: 32px;
  object-fit: contain;
}

.workspace-column {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  overflow-y: auto;
  padding: 8px;
  min-height: 0;
}

.workspace-avatar-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
}

.workspace-remove-btn {
  position: absolute;
  bottom: -12px;
  right: -8px;
  z-index: 1;
  opacity: 0;
  transform: scale(0.4);
  transition: all 0.15s ease;
}

.workspace-avatar-wrapper:hover .workspace-remove-btn {
  opacity: 1;
  transform: scale(1);
}

.workspace-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  color: #fff;
  font-weight: 700;
  line-height: 1;
  transition: all 0.2s ease;
  user-select: none;
  flex-shrink: 0;
}

.workspace-avatar:hover {
  opacity: 0.85;
  transform: scale(1.12);
}

.workspace-avatar.active {
  opacity: 1;
  transform: scale(1);
}

.session-item-wrapper {
  position: relative;
}

.session-remove-btn {
  position: absolute;
  top: 2px;
  right: 2px;
  z-index: 1;
  opacity: 0;
  transform: scale(0.4);
  transition: all 0.15s ease;
}

.session-item-wrapper:hover .session-remove-btn {
  opacity: 1;
  transform: scale(1);
}
</style>
