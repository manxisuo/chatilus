<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import InspectorSection from "./InspectorSection.vue";
import type { ConversationSummary, MessageView } from "../types";
import { sourceLabel, sourceAccentColor } from "../utils/dataSource";
import { formatSourceContextLine, sourceContextIdHint } from "../utils/sourceContext";
import { type AppLocale, formatDateTime } from "../utils/locale";

const props = defineProps<{
  conversation: ConversationSummary;
  messages: MessageView[];
}>();

const { t, locale } = useI18n();

function formatTime(timestamp: number | null) {
  return formatDateTime(timestamp, locale.value as AppLocale);
}

const imageCount = computed(() =>
  props.messages.reduce((sum, message) => sum + message.attachments.length, 0),
);

const codeMessageCount = computed(() =>
  props.messages.filter((message) => message.content.includes("```")).length,
);

const timeSpan = computed(() => {
  const times = props.messages
    .map((message) => message.create_time)
    .filter((value): value is number => value != null);
  if (times.length === 0) {
    return {
      start: props.conversation.create_time,
      end: props.conversation.update_time,
    };
  }
  return {
    start: Math.min(...times),
    end: Math.max(...times),
  };
});
</script>

<template>
  <aside class="conv-info" :aria-label="t('conversation.infoAria')">
    <h3 class="panel-title">{{ t("conversation.info") }}</h3>

    <InspectorSection :title="t('conversation.sectionBasic')">
      <dl class="cl-inspector-props">
        <div class="cl-inspector-row">
          <dt>{{ t("conversation.source") }}</dt>
          <dd class="source-value">
            <span
              class="source-dot"
              :style="{ background: sourceAccentColor(conversation.source) }"
            />
            {{ sourceLabel(conversation.source ?? "chatgpt") }}
          </dd>
        </div>
        <div v-if="conversation.model" class="cl-inspector-row">
          <dt>{{ t("conversation.model") }}</dt>
          <dd>{{ conversation.model }}</dd>
        </div>
        <div class="cl-inspector-row">
          <dt>{{ t("conversation.messageCount") }}</dt>
          <dd>{{ t("common.messages", { count: conversation.message_count }) }}</dd>
        </div>
        <div class="cl-inspector-row">
          <dt>{{ t("conversation.imageCount") }}</dt>
          <dd>{{ t("common.images", { count: imageCount }) }}</dd>
        </div>
        <div v-if="codeMessageCount > 0" class="cl-inspector-row">
          <dt>{{ t("conversation.withCode") }}</dt>
          <dd>{{ t("conversation.codeMessages", { count: codeMessageCount }) }}</dd>
        </div>
        <div class="cl-inspector-row">
          <dt>{{ t("conversation.started") }}</dt>
          <dd>{{ formatTime(timeSpan.start) }}</dd>
        </div>
        <div class="cl-inspector-row">
          <dt>{{ t("conversation.updated") }}</dt>
          <dd>{{ formatTime(timeSpan.end) }}</dd>
        </div>
      </dl>
    </InspectorSection>

    <InspectorSection
      v-if="conversation.source_contexts?.length || conversation.tags.length"
      :title="t('conversation.sectionContext')"
    >
      <dl class="cl-inspector-props">
        <div v-if="conversation.source_contexts?.length" class="cl-inspector-row">
          <dt>{{ t("conversation.sourceContext") }}</dt>
          <dd class="context-list">
            <span
              v-for="context in conversation.source_contexts"
              :key="context.id"
              class="context-line"
            >
              <span>{{ formatSourceContextLine(context) }}</span>
              <span v-if="sourceContextIdHint(context)" class="context-id">
                {{ sourceContextIdHint(context) }}
              </span>
            </span>
          </dd>
        </div>
        <div v-if="conversation.tags.length > 0" class="cl-inspector-row">
          <dt>{{ t("conversation.tags") }}</dt>
          <dd class="tag-list">
            <span v-for="tag in conversation.tags" :key="tag" class="tag-pill">{{ tag }}</span>
          </dd>
        </div>
      </dl>
    </InspectorSection>
  </aside>
</template>

<style scoped>
.conv-info {
  width: var(--cl-insight-width);
  flex-shrink: 0;
  border-left: 1px solid var(--cl-border-subtle);
  background: var(--cl-panel);
  padding: 16px;
  overflow: auto;
}

.panel-title {
  margin: 0 0 12px;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--cl-text-faint);
}

.source-value {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.source-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.tag-pill {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--cl-selected);
  color: var(--cl-text-muted);
}

.context-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.context-line {
  color: var(--cl-text-muted);
  font-size: 11px;
  line-height: 1.4;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.context-id {
  font-family: ui-monospace, monospace;
  font-size: 10px;
  word-break: break-all;
  opacity: 0.85;
}

@media (max-width: 1100px) {
  .conv-info {
    display: none;
  }
}
</style>
