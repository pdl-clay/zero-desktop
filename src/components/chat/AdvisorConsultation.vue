<template>
  <div :class="['tool-call-card q-mb-sm', cardClass]">
    <div class="row items-center q-px-sm q-py-xs">
      <q-spinner-dots
        v-if="message.status === 'running'"
        size="14px"
        color="info"
        class="q-mr-sm"
      />
      <q-icon
        v-else
        :name="isError ? 'error' : 'check_circle'"
        size="14px"
        :color="isError ? 'negative' : 'positive'"
        class="q-mr-xs"
      />
      <q-icon name="auto_awesome" size="14px" color="purple-4" class="q-mr-xs" />
      <span class="text-caption text-weight-medium tool-name">{{
        $t("chat.advisorConsultationTitle")
      }}</span>
      <span class="text-caption text-grey-6 q-ml-xs">{{ statusLabel }}</span>
      <q-tooltip
        v-if="message.status === 'running' && message.prompt"
        max-width="400px"
        anchor="bottom left"
        self="top left"
      >
        <pre class="tool-input-preview">{{ message.prompt }}</pre>
      </q-tooltip>
      <q-space />
      <q-btn
        v-if="isDone && message.content"
        round
        dense
        flat
        size="xs"
        :icon="showResult ? 'expand_less' : 'expand_more'"
        color="grey-5"
        @click="showResult = !showResult"
      >
        <q-tooltip>{{ showResult ? $t("chat.showLess") : $t("chat.showMore") }}</q-tooltip>
      </q-btn>
      <q-btn
        v-if="isDone && message.content"
        round
        dense
        flat
        size="xs"
        icon="content_copy"
        color="grey-5"
        @click="onCopy"
      >
        <q-tooltip>{{ $t("chat.copy") }}</q-tooltip>
      </q-btn>
    </div>

    <div v-if="showResult && message.content" class="tool-result-body q-px-sm q-pb-sm">
      <div class="md-chat-message" v-html="renderedContent" />
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from "vue";
import { useQuasar, copyToClipboard } from "quasar";
import { useI18n } from "vue-i18n";
import { renderMarkdown } from "@/utils/markdown";

const props = defineProps({
  message: {
    type: Object,
    required: true,
  },
});

const $q = useQuasar();
const { t: $t } = useI18n();
// Starts collapsed, same as ToolCallMessage.vue's tool cards - an advisor
// consultation is a specialist sub-agent call under the hood (Task tool
// with name: "advisor"), so it should read as the same family of
// collapsible tool cards, not a special always-expanded block.
const showResult = ref(false);

const isError = computed(() => props.message.status === "error");
const isDone = computed(() => props.message.status === "completed" || isError.value);

const cardClass = computed(() => ({
  "tool-call-card--dark": $q.dark.isActive,
  "tool-call-card--running": props.message.status === "running",
  "tool-call-card--completed": props.message.status === "completed",
  "tool-call-card--error": isError.value,
}));

const statusLabel = computed(() => {
  if (props.message.status === "running") return $t("chat.toolRunning");
  return isError.value ? $t("chat.toolFailed") : $t("chat.toolCompleted");
});

const renderedContent = computed(() => renderMarkdown(props.message.content || ""));

function onCopy() {
  copyToClipboard(props.message.content || "");
}
</script>
