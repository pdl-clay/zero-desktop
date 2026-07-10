<template>
  <q-page class="column">
    <!-- Error banner -->
    <q-banner v-if="zeroStore.zeroError" class="bg-negative text-white" dense rounded>
      <template v-slot:action>
        <q-btn flat dense label="OK" @click="zeroStore.zeroError = null" />
      </template>
      {{ zeroStore.zeroError }}
    </q-banner>

    <!-- Messages -->
    <div class="col q-pa-md scroll" ref="messagesContainer">
      <div
        v-if="zeroStore.messages.length === 0 && !zeroStore.currentResponse"
        class="flex flex-center full-height text-grey-6 text-center"
      >
        <div>
          <img src="/zero-completa.png" alt="Zero" style="width: auto; height: auto; margin-bottom: 8px" />
          <div class="text-body1">Envie uma mensagem para iniciar</div>
          <div class="text-caption">uma nova sessão</div>
        </div>
      </div>

      <div
        v-for="(message, index) in zeroStore.messages"
        :key="index"
        class="q-mb-sm"
      >
        <q-chat-message
          :name="message.role === 'user' ? 'Você' : 'Zero'"
          :text="[message.content]"
          :sent="message.role === 'user'"
          :bg-color="messageBubbleColor(message.role)"
          :text-color="message.role === 'user' ? 'white' : undefined"
        />
      </div>

      <!-- Streaming response -->
      <q-chat-message
        v-if="zeroStore.currentResponse"
        name="Zero"
        :text="[zeroStore.currentResponse]"
        :bg-color="$q.dark.isActive ? 'grey-9' : 'grey-3'"
      />
    </div>

    <!-- Input -->
    <div :class="$q.dark.isActive ? 'bg-dark q-pa-xs' : 'bg-grey-1 q-pa-xs'">
      <q-input
        v-model="input"
        filled
        dense
        type="textarea"
        autogrow
        :placeholder="$t('chat.placeholder')"
        @keydown.enter.prevent="onSend"
        :disable="!zeroStore.isConnected"
        bottom-slots
      >
        <template v-slot:after>
          <q-btn
            round
            dense
            flat
            icon="send"
            color="primary"
            :disable="!input.trim() || !zeroStore.isConnected"
            @click="onSend"
          />
        </template>
      </q-input>
    </div>
  </q-page>
</template>

<script setup>
import { ref, watch, nextTick } from 'vue'
import { useQuasar } from 'quasar'
import { useZeroStore } from '@/stores/zero-store'

const props = defineProps({
  workspacePath: {
    type: String,
    required: true,
  },
})

const $q = useQuasar()
const zeroStore = useZeroStore()
const input = ref('')
const messagesContainer = ref(null)

watch(
  () => zeroStore.messages.length,
  () => {
    nextTick(() => {
      if (messagesContainer.value) {
        messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
      }
    })
  },
)

function messageBubbleColor(role) {
  switch (role) {
    case 'user':
      return 'primary'
    case 'assistant':
      return $q.dark.isActive ? 'grey-9' : 'grey-3'
    case 'system':
      return 'info'
    default:
      return 'warning'
  }
}

async function onSend() {
  const content = input.value.trim()
  if (!content) return

  input.value = ''
  await zeroStore.sendMessage(content)
}
</script>
