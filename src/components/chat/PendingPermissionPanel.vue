<template>
  <div class="permission-panel q-mb-sm">
    <div class="permission-panel__header row items-center q-pa-sm">
      <q-icon :name="statusIcon" :color="statusColor" size="18px" class="q-mr-sm" />
      <span class="text-body2 text-weight-medium">{{ statusLabel }}</span>
    </div>
    <div class="q-px-sm q-pb-sm">
      <div class="text-caption text-grey-6 q-mb-xs">
        <strong>{{ request.toolName }}</strong>
      </div>
      <div v-if="request.reason" class="text-caption q-mb-xs permission-panel__reason">
        {{ request.reason }}
      </div>
      <pre v-if="request.proposedCommand" class="permission-panel__command">{{
        request.proposedCommand
      }}</pre>

      <div class="row q-mt-sm q-gutter-sm">
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
  request: { type: Object, required: true },
});

const { t: $t } = useI18n();
const zeroStore = useZeroStore();

const statusIcon = computed(() => "gpp_maybe");
const statusColor = computed(() => "warning");
const statusLabel = computed(() => $t("chat.permissionRequired"));

function onApprove() {
  zeroStore.approvePermission(props.request.permissionId);
}

function onDeny() {
  zeroStore.denyPermission(props.request.permissionId);
}
</script>

<style scoped>
.permission-panel {
  border-radius: 12px;
  border: 1px solid rgba(255, 152, 0, 0.35);
  background: rgba(255, 152, 0, 0.08);
  color: var(--chat-text);
}
body.body--dark .permission-panel {
  border-color: rgba(255, 152, 0, 0.25);
  background: rgba(255, 152, 0, 0.05);
}
.permission-panel__reason {
  color: var(--chat-text-muted);
}
.permission-panel__command {
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
