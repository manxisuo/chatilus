<script setup lang="ts">
import { computed } from "vue";
import type { ConversationSummary, MessageView } from "../types";
import { sourceLabel, sourceTagType } from "../utils/dataSource";

const props = defineProps<{
  conversation: ConversationSummary;
  messages: MessageView[];
}>();

function formatTime(timestamp: number | null) {
  if (!timestamp) return "—";
  return new Date(timestamp * 1000).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
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
  <aside class="conv-info" aria-label="对话信息">
    <h3 class="title">对话信息</h3>
    <dl class="info-list">
      <div class="info-row">
        <dt>来源</dt>
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
        <dt>模型</dt>
        <dd>{{ conversation.model }}</dd>
      </div>
      <div class="info-row">
        <dt>消息</dt>
        <dd>{{ conversation.message_count }} 条</dd>
      </div>
      <div class="info-row">
        <dt>图片</dt>
        <dd>{{ imageCount }} 张</dd>
      </div>
      <div v-if="codeMessageCount > 0" class="info-row">
        <dt>含代码</dt>
        <dd>{{ codeMessageCount }} 条消息</dd>
      </div>
      <div class="info-row">
        <dt>开始</dt>
        <dd>{{ formatTime(timeSpan.start) }}</dd>
      </div>
      <div class="info-row">
        <dt>最近</dt>
        <dd>{{ formatTime(timeSpan.end) }}</dd>
      </div>
      <div v-if="conversation.tags.length > 0" class="info-row">
        <dt>标签</dt>
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

@media (max-width: 1100px) {
  .conv-info {
    display: none;
  }
}
</style>
