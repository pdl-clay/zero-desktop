<template>
  <q-dialog :model-value="!!review" persistent @update:model-value="() => {}">
    <div v-if="review" class="plan-review">
      <div class="plan-review__header row items-center no-wrap">
        <span class="plan-review__badge">
          <q-icon name="fact_check" size="16px" />
        </span>
        <div class="plan-review__heading column">
          <span class="plan-review__eyebrow">{{ $t("chat.planReviewTitle") }}</span>
          <span class="plan-review__title ellipsis">{{
            review.title || $t("chat.planReviewTitle")
          }}</span>
        </div>
      </div>

      <div class="plan-review__separator" />

      <div class="plan-review__body">
        <div class="md-chat-message plan-review__markdown" v-html="renderedContent" />
      </div>

      <template v-if="showFeedback">
        <div class="plan-review__separator" />
        <div class="plan-review__feedback">
          <textarea
            ref="feedbackRef"
            v-model="feedback"
            class="plan-review__textarea"
            rows="1"
            :placeholder="$t('chat.planReviewFeedbackPlaceholder')"
            @input="autoResize"
          />
        </div>
      </template>

      <div class="plan-review__separator" />

      <div class="plan-review__actions">
        <template v-if="!showFeedback">
          <button
            type="button"
            class="plan-review__btn plan-review__btn--ghost"
            @click="showFeedback = true"
          >
            {{ $t("chat.planReviewRequestChanges") }}
          </button>
          <button
            type="button"
            class="plan-review__btn plan-review__btn--outline"
            @click="onApprove('ask')"
          >
            {{ $t("chat.planReviewApproveAsk") }}
          </button>
          <button
            type="button"
            class="plan-review__btn plan-review__btn--primary"
            @click="onApprove('auto')"
          >
            {{ $t("chat.planReviewApproveAuto") }}
          </button>
        </template>
        <template v-else>
          <button
            type="button"
            class="plan-review__btn plan-review__btn--ghost"
            @click="showFeedback = false"
          >
            {{ $t("common.cancel") }}
          </button>
          <button
            type="button"
            class="plan-review__btn plan-review__btn--primary"
            :disabled="!feedback.trim()"
            @click="onRequestChanges"
          >
            {{ $t("chat.planReviewSendFeedback") }}
          </button>
        </template>
      </div>
    </div>
  </q-dialog>
</template>

<script setup>
import { ref, computed, inject, watch, nextTick } from "vue";
import { renderMarkdown } from "@/utils/markdown";

const sessionStore = inject("zeroStore");

const review = computed(() => sessionStore.pendingPlanReview);
const renderedContent = computed(() => renderMarkdown(review.value?.content || ""));

const showFeedback = ref(false);
const feedback = ref("");
const feedbackRef = ref(null);

const MAX_TEXTAREA_HEIGHT = 160;

function autoResize() {
  const el = feedbackRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, MAX_TEXTAREA_HEIGHT)}px`;
}

// A new plan (or the dialog closing) should never carry over a stale
// half-typed feedback draft from the previous review.
watch(review, () => {
  showFeedback.value = false;
  feedback.value = "";
});

watch(showFeedback, (open) => {
  if (open) nextTick(() => feedbackRef.value?.focus());
});

function onApprove(mode) {
  sessionStore.approvePlanReview(mode);
}

function onRequestChanges() {
  if (!feedback.value.trim()) return;
  sessionStore.requestPlanChanges(feedback.value);
}
</script>

<style scoped>
/* Same "glass pill" language as ChatInput.vue's mode/advisor dropdowns and
   ModelPickerDropdown.vue (blurred dark panel + subtle border + pill
   controls) - this dialog is deliberately built out of the same tokens
   instead of stock Quasar card/button chrome, so it reads as the same
   design system rather than a one-off. Opacity/blur are pushed higher than
   those transient menus (0.5/14px there vs 0.86/22px here) because this
   panel carries a full markdown plan to actually read, not a 3-line list -
   at that opacity the panel is visually dark regardless of the app's
   light/dark theme, so text colors below are fixed light tones rather than
   the theme-flipping var(--chat-text) tokens those lighter menus use.
*/
.plan-review {
  width: 720px;
  max-width: 90vw;
  display: flex;
  flex-direction: column;
  border-radius: 20px;
  border: 1px solid rgba(128, 128, 128, 0.18);
  background: rgba(22, 22, 24, 0.86);
  backdrop-filter: blur(22px);
  -webkit-backdrop-filter: blur(22px);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.34);
  overflow: hidden;
  color: rgba(255, 255, 255, 0.92);
}

.plan-review__header {
  gap: 10px;
  padding: 16px 20px;
}

.plan-review__badge {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: rgba(25, 118, 210, 0.16);
  color: #64b5f6;
}

.plan-review__heading {
  min-width: 0;
  gap: 1px;
}

.plan-review__eyebrow {
  font-size: 0.72em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  color: rgba(255, 255, 255, 0.55);
}

.plan-review__title {
  font-size: 1.02em;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
}

.plan-review__separator {
  height: 1px;
  background: rgba(128, 128, 128, 0.18);
}

.plan-review__body {
  max-height: 50vh;
  overflow-y: auto;
  padding: 16px 20px;
}

.plan-review__feedback {
  padding: 14px 20px;
}

.plan-review__textarea {
  width: 100%;
  resize: none;
  border: none;
  outline: none;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.07);
  border: 1px solid rgba(255, 255, 255, 0.14);
  padding: 10px 12px;
  font-family: inherit;
  font-size: 0.9em;
  line-height: 1.4;
  max-height: 160px;
  overflow-y: auto;
  color: rgba(255, 255, 255, 0.92);
}

.plan-review__textarea::placeholder {
  color: rgba(255, 255, 255, 0.4);
}

.plan-review__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
}

/* Pill button recipe mirrors .chat-input__mode / .model-picker__button
   (ghost + primary-tinted states) and .chat-input__send--active (solid
   primary), just tuned to this card's dark surface. */
.plan-review__btn {
  height: 36px;
  padding: 0 16px;
  border-radius: 18px;
  border: 1px solid transparent;
  font-size: 0.85em;
  font-weight: 500;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease,
    transform 0.1s ease,
    opacity 0.15s ease;
}

.plan-review__btn:active:not(:disabled) {
  transform: scale(0.97);
}

.plan-review__btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.plan-review__btn--ghost {
  background: transparent;
  border-color: rgba(255, 255, 255, 0.18);
  color: rgba(255, 255, 255, 0.75);
}

.plan-review__btn--ghost:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.28);
}

.plan-review__btn--outline {
  background: rgba(25, 118, 210, 0.1);
  border-color: rgba(25, 118, 210, 0.4);
  color: #64b5f6;
}

.plan-review__btn--outline:hover:not(:disabled) {
  background: rgba(25, 118, 210, 0.18);
  border-color: rgba(25, 118, 210, 0.5);
}

.plan-review__btn--primary {
  background: var(--q-primary, #1976d2);
  border-color: transparent;
  color: white;
}

.plan-review__btn--primary:hover:not(:disabled) {
  filter: brightness(1.08);
}

/* The markdown plan body reuses the app-wide .md-chat-message rules (app.scss)
   for structure (headings/lists/tables/code), but overrides its theme-flipping
   var(--chat-text) color - this card is always dark, in both app themes. */
.plan-review__markdown {
  font-size: 0.92em;
}

.plan-review__markdown:deep(.q-message-text-content),
.plan-review__markdown {
  color: rgba(255, 255, 255, 0.88);
}

.plan-review__markdown:deep(code:not(.hljs)) {
  background: rgba(255, 255, 255, 0.12);
}

.plan-review__markdown:deep(pre.md-code) {
  background: rgba(255, 255, 255, 0.08);
}

.plan-review__markdown:deep(th) {
  background: rgba(255, 255, 255, 0.1);
}

.plan-review__markdown:deep(th),
.plan-review__markdown:deep(td) {
  border-color: rgba(255, 255, 255, 0.18);
}

.plan-review__markdown:deep(hr) {
  border-top-color: rgba(255, 255, 255, 0.18);
}

.plan-review__markdown:deep(blockquote) {
  border-left-color: rgba(255, 255, 255, 0.3);
  color: rgba(255, 255, 255, 0.75);
}

.plan-review__markdown:deep(a) {
  color: #64b5f6;
}
</style>
