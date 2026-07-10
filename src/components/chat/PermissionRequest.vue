<template>
  <div class="permission-card q-mb-sm">
    <div class="row items-center q-px-sm q-py-xs">
      <q-icon :name="statusIcon" :color="statusColor" size="18px" class="q-mr-sm" />
      <span class="text-body2 text-weight-medium">{{ statusLabel }}</span>
    </div>
    <div class="q-px-sm q-pb-sm">
      <div class="text-caption text-grey-6 q-mb-xs">
        <strong>{{ message.toolName }}</strong>
      </div>
      <div v-if="message.reason" class="text-caption q-mb-xs permission-reason">
        {{ message.reason }}
      </div>
      <pre v-if="message.proposedCommand" class="permission-command">{{
        message.proposedCommand
      }}</pre>

      <div v-if="message.status === 'pending'" class="row q-mt-sm q-gutter-sm">
        <q-btn
          outline
          dense
          color="positive"
          icon="check"
          :label="$t('chat.approve')"
          @click="onApprove"
        />
        <q-btn
          outline
          dense
          color="negative"
          icon="close"
          :label="$t('chat.deny')"
          @click="onDeny"
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useZeroStore } from "@/stores/zero-store";

const props = defineProps({
  message: { type: Object, required: true },
});

const { t: $t } = useI18n();
const zeroStore = useZeroStore();

const statusIcon = computed(() => {
  switch (props.message.status) {
    case "approved":
      return "check_circle";
    case "denied":
      return "cancel";
    default:
      return "gpp_maybe";
  }
});

const statusColor = computed(() => {
  switch (props.message.status) {
    case "approved":
      return "positive";
    case "denied":
      return "negative";
    default:
      return "warning";
  }
});

const statusLabel = computed(() => {
  switch (props.message.status) {
    case "approved":
      return $t("chat.permissionApproved");
    case "denied":
      return $t("chat.permissionDenied");
    default:
      return $t("chat.permissionRequired");
  }
});

function onApprove() {
  zeroStore.approvePermission(props.message.permissionId);
}

function onDeny() {
  zeroStore.denyPermission(props.message.permissionId);
}
</script>

<style scoped>
.permission-card {
  border-radius: 8px;
  border: 1px solid rgba(255, 152, 0, 0.3);
  background: rgba(255, 152, 0, 0.06);
  color: var(--chat-text);
}
body.body--dark .permission-card {
  border-color: rgba(255, 152, 0, 0.2);
  background: rgba(255, 152, 0, 0.04);
}
.permission-reason {
  color: var(--chat-text-muted);
}
.permission-command {
  margin: 0;
  font-size: 0.82em;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
  background: var(--chat-card-bg);
  color: var(--chat-text);
  padding: 8px;
  border-radius: 6px;
}
</style>
