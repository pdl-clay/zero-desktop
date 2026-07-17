<template>
  <div class="session-item-wrapper" :style="depth ? { paddingLeft: `${depth * 14}px` } : null">
    <q-item
      clickable
      v-ripple
      :class="[
        'session-item q-px-sm',
        {
          'session-item--active': actions.isSessionOpen(session),
        },
      ]"
      @click="actions.onSelectSession(session)"
    >
      <q-item-section side>
        <SessionIndicator
          :status="actions.sessionWorkingStatus(session)"
          :attention="actions.sessionAttention(session)"
          :size="18"
        />
      </q-item-section>

      <q-item-section class="session-item__content">
        <q-item-label class="text-body2 session-item__title" lines="1">
          {{ actions.truncateTitle(session.title) || session.session_id.slice(-8) }}
          <q-tooltip v-if="session.title" anchor="top middle" self="bottom middle">
            {{ session.title }}
          </q-tooltip>
        </q-item-label>
        <q-item-label caption class="row items-center q-gutter-x-xs session-item__meta">
          <span v-if="depth > 0" class="session-item__origin ellipsis">
            {{ session.agent_name || session.kind || $t("workspace.subagentFallback") }}
          </span>
          <span v-if="session.model_id" class="ellipsis">{{ session.model_id }}</span>
          <span v-if="session.model_id">&middot;</span>
          <span class="ellipsis">{{ actions.formatDate(session.created_at) }}</span>
        </q-item-label>
      </q-item-section>
    </q-item>

    <q-btn
      class="session-rename-btn"
      round
      dense
      flat
      size="xs"
      icon="edit"
      color="grey-7"
      @click.stop="actions.onRenameSession(session)"
    >
      <q-tooltip>{{ $t("workspace.renameSession") }}</q-tooltip>
    </q-btn>
    <q-btn
      class="session-remove-btn"
      round
      dense
      flat
      size="xs"
      icon="close"
      color="negative"
      @click.stop="actions.onDeleteSession(session)"
    >
      <q-tooltip>{{ $t("workspace.deleteSession") }}</q-tooltip>
    </q-btn>

    <button
      v-if="session.children?.length"
      type="button"
      class="session-children-toggle"
      @click="expanded = !expanded"
    >
      <q-icon :name="expanded ? 'expand_less' : 'expand_more'" size="15px" />
      <span>{{ $t("workspace.subagentSessions", { count: session.children.length }) }}</span>
    </button>

    <div v-if="expanded && session.children?.length" class="session-children">
      <SessionListItem
        v-for="child in session.children"
        :key="child.session_id"
        :session="child"
        :depth="depth + 1"
      />
    </div>
  </div>
</template>

<script setup>
import { ref, inject } from "vue";
import SessionIndicator from "@/components/SessionIndicator.vue";

defineProps({
  session: { type: Object, required: true },
  depth: { type: Number, default: 0 },
});

// Provided by MainLayout.vue - the handlers/formatters the flat session list
// used to close over directly. Injected (not prop-drilled) so every level of
// this recursive component doesn't need to re-declare and re-pass the same
// eight functions down the chain.
const actions = inject("sessionListActions");
// Collapsed by default so the sidebar stays clean - subagent/advisor
// sessions are one click away, not cluttering the list up front.
const expanded = ref(false);
</script>

<style scoped>
.session-item-wrapper {
  position: relative;
  min-width: 0;
  width: 100%;
}

.session-remove-btn,
.session-rename-btn {
  position: absolute;
  top: 2px;
  z-index: 1;
  opacity: 0;
  transform: scale(0.4);
  transition: all 0.15s ease;
}

.session-remove-btn {
  right: 2px;
}

.session-rename-btn {
  right: 24px;
}

.session-item-wrapper:hover .session-remove-btn,
.session-item-wrapper:hover .session-rename-btn {
  opacity: 1;
  transform: scale(1);
}

.session-item-wrapper :deep(.q-item__section--main) {
  min-width: 0;
  overflow: hidden;
}

.session-item__content {
  min-width: 0 !important;
  overflow: hidden;
}

.session-item__title,
.session-item__meta {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-item__meta span {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Small origin tag for a subagent/advisor row - same pill language as
   .chat-input__mode-item-desc (secondary text) in ChatInput.vue, just
   wrapped in a chip instead of run inline, since this sits in a dense list
   row rather than a dropdown item. */
.session-item__origin {
  flex-shrink: 0;
  padding: 1px 7px;
  border-radius: 8px;
  background: rgba(128, 128, 128, 0.14);
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.85));
  font-size: 0.85em;
  line-height: 1.5;
}

/* Same pill family as .chat-input__mode / .chat-input__advisor-pill: 1px
   neutral border, transparent fill at rest, blue "active" accent - so a row
   open in a live panel reads as the same kind of "on" state as an active
   toggle in the input bar, not a different color language. */
.session-item {
  border-radius: 10px;
  border: 1px solid transparent;
  color: var(--chat-text);
  width: 100%;
  min-width: 0;
  transition:
    background 0.15s ease,
    border-color 0.15s ease;
}

.session-item:hover {
  background: rgba(128, 128, 128, 0.08);
  border-color: rgba(128, 128, 128, 0.22);
}

.session-item--active {
  color: var(--q-primary, #1976d2);
  background: rgba(25, 118, 210, 0.06);
  border-color: rgba(25, 118, 210, 0.35);
}

.session-item--active:hover {
  background: rgba(25, 118, 210, 0.12);
  border-color: rgba(25, 118, 210, 0.45);
}

/* Same shape/hover treatment as .chat-input__mode-item (the pills dropdown's
   list rows): rounded, transparent at rest, neutral hover fill. */
.session-children-toggle {
  display: flex;
  align-items: center;
  gap: 5px;
  width: fit-content;
  margin-top: 2px;
  padding: 3px 10px 3px 6px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: var(--chat-text-muted, rgba(128, 128, 128, 0.85));
  font-size: 0.78em;
  font-weight: 500;
  cursor: pointer;
  transition:
    background 0.12s ease,
    color 0.12s ease;
}

.session-children-toggle:hover {
  background: rgba(128, 128, 128, 0.1);
  color: var(--chat-text);
}

.session-children {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
</style>
