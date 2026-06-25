<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ConversationSummary, MessageView } from "../types";
import { sourceLabel, sourceTagType } from "../utils/dataSource";
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
    <h3 class="title">{{ t("conversation.info") }}</h3>
    <dl class="info-list">
      <div class="info-row">
        <dt>{{ t("conversation.source") }}</dt>
        <dd>
          <el-tag
            size="small"
            :type="sourceTagType(conversation.source ?? 'chatgpt')"
            effect="plain"
          >
            {{ sourceLabel(conversation.source ?? "chatgpt") }}
          </el-tag>
        </dd>
      </div>
      <div v-if="conversation.model" class="info-row">
        <dt>{{ t("conversation.model") }}</dt>
        <dd>{{ conversation.model }}</dd>
      </div>
      <div class="info-row">
        <dt>{{ t("conversation.messageCount") }}</dt>
        <dd>{{ t("common.messages", { count: conversation.message_count }) }}</dd>
      </div>
      <div class="info-row">
        <dt>{{ t("conversation.imageCount") }}</dt>
        <dd>{{ t("common.images", { count: imageCount }) }}</dd>
      </div>
      <div v-if="codeMessageCount > 0" class="info-row">
        <dt>{{ t("conversation.withCode") }}</dt>
        <dd>{{ t("conversation.codeMessages", { count: codeMessageCount }) }}</dd>
      </div>
      <div class="info-row">
        <dt>{{ t("conversation.started") }}</dt>
        <dd>{{ formatTime(timeSpan.start) }}</dd>
      </div>
      <div class="info-row">
        <dt>{{ t("conversation.updated") }}</dt>
        <dd>{{ formatTime(timeSpan.end) }}</dd>
      </div>
      <div v-if="conversation.source_contexts?.length" class="info-row">
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
      <div v-if="conversation.tags.length > 0" class="info-row">
        <dt>{{ t("conversation.tags") }}</dt>
        <dd class="tag-list">
          <el-tag
            v-for="tag in conversation.tags"
            :key="tag"
            size="small"
            type="info"
            effect="plain"
          >
            {{ tag }}
          </el-tag>
        </dd>
      </div>
    </dl>
  </aside>
</template>

<style scoped>
.conv-info {
  width: 240px;
  flex-shrink: 0;
  border-left: 1px solid var(--cl-border);
  background: var(--cl-bg);
  padding: 16px;
  overflow: auto;
}

.title {
  margin: 0 0 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text-muted);
}

.info-list {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.info-row {
  display: grid;
  grid-template-columns: 52px 1fr;
  gap: 8px;
  align-items: start;
  font-size: 12px;
}

.info-row dt {
  margin: 0;
  color: var(--cl-text-muted);
  font-weight: 500;
}

.info-row dd {
  margin: 0;
  color: var(--cl-text);
  word-break: break-word;
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
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
  font-family: var(--el-font-family-monospace, ui-monospace, monospace);
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
