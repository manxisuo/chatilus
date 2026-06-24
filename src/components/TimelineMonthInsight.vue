<script setup lang="ts">
import { computed } from "vue";
import type { SourceCount } from "../types";
import type { TopicBubble, TopicFilter } from "../utils/timelineTopics";
import { isSameTopicFilter } from "../utils/timelineTopics";
import { sourceLabel, sourceTagType } from "../utils/dataSource";

const props = defineProps<{
  monthLabel: string | null;
  loadedCount: number;
  totalCount: number;
  imageCount: number;
  messageCount: number;
  starredCount: number;
  activeDays: number;
  sourceCounts: SourceCount[];
  topics: TopicBubble[];
  loading: boolean;
  activeTopicFilter: TopicFilter | null;
}>();

const emit = defineEmits<{
  openGallery: [];
  selectTopic: [topic: TopicFilter];
}>();

const isPartial = computed(() => props.loadedCount < props.totalCount);

const sourceRows = computed(() => {
  const total = props.sourceCounts.reduce((sum, item) => sum + item.count, 0);
  if (total <= 0) return [];
  return props.sourceCounts.map((item) => ({
    ...item,
    percent: Math.round((item.count / total) * 100),
  }));
});
</script>

<template>
  <aside class="month-insight" aria-label="本月洞察">
    <template v-if="monthLabel">
      <h3 class="insight-title">{{ monthLabel }}</h3>
      <p v-if="isPartial" class="insight-hint">
        基于已加载 {{ loadedCount }} / {{ totalCount }} 条对话
      </p>

      <dl class="insight-stats">
        <div class="stat-row">
          <dt>对话</dt>
          <dd>{{ totalCount.toLocaleString() }}</dd>
        </div>
        <div class="stat-row">
          <dt>消息</dt>
          <dd>{{ messageCount.toLocaleString() }}</dd>
        </div>
        <div class="stat-row">
          <dt>活跃天</dt>
          <dd>{{ activeDays }}</dd>
        </div>
        <div v-if="starredCount > 0" class="stat-row">
          <dt>收藏</dt>
          <dd>{{ starredCount }}</dd>
        </div>
        <div v-if="imageCount > 0" class="stat-row">
          <dt>图片</dt>
          <dd>
            <button type="button" class="insight-link" @click="emit('openGallery')">
              {{ imageCount }} 张
            </button>
          </dd>
        </div>
      </dl>

      <section v-if="sourceRows.length" class="insight-section">
        <h4 class="section-label">来源分布</h4>
        <ul class="source-bars">
          <li v-for="item in sourceRows" :key="item.source" class="source-bar-item">
            <div class="source-bar-head">
              <el-tag size="small" :type="sourceTagType(item.source)" effect="plain">
                {{ sourceLabel(item.source) }}
              </el-tag>
              <span class="source-bar-count">{{ item.count }} · {{ item.percent }}%</span>
            </div>
            <div class="source-bar-track">
              <div class="source-bar-fill" :style="{ width: `${item.percent}%` }" />
            </div>
          </li>
        </ul>
      </section>

      <section class="insight-section">
        <h4 class="section-label">主要话题</h4>
        <el-skeleton v-if="loading && topics.length === 0" animated :rows="3" />
        <p v-else-if="topics.length === 0" class="empty-topics">暂无足够数据提取话题</p>
        <div v-else class="topic-bubbles">
          <button
            v-for="topic in topics"
            :key="`${topic.kind}:${topic.label}`"
            type="button"
            class="topic-bubble"
            :class="[topic.kind, { active: isSameTopicFilter(activeTopicFilter, topic) }]"
            @click="emit('selectTopic', { label: topic.label, kind: topic.kind })"
          >
            <span class="topic-label">{{ topic.label }}</span>
            <span class="topic-count">{{ topic.count }}</span>
          </button>
        </div>
      </section>
    </template>
    <p v-else class="insight-placeholder">滚动或点击左侧月份查看本月洞察</p>
  </aside>
</template>

<style scoped>
.month-insight {
  width: var(--cl-insight-width);
  flex-shrink: 0;
  border-left: 1px solid var(--cl-border);
  background: var(--cl-bg);
  padding: 16px;
  overflow: auto;
}

.insight-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.insight-hint {
  margin: 6px 0 0;
  font-size: 11px;
  color: var(--cl-text-muted);
}

.insight-placeholder {
  margin: 0;
  font-size: 12px;
  color: var(--cl-text-muted);
  line-height: 1.5;
}

.insight-stats {
  margin: 14px 0 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.stat-row {
  display: grid;
  grid-template-columns: 52px 1fr;
  gap: 8px;
  font-size: 12px;
}

.stat-row dt {
  margin: 0;
  color: var(--cl-text-muted);
  font-weight: 500;
}

.stat-row dd {
  margin: 0;
  color: var(--cl-text);
  font-variant-numeric: tabular-nums;
}

.insight-link {
  border: none;
  background: transparent;
  padding: 0;
  font-size: 12px;
  color: var(--el-color-primary);
  cursor: pointer;
}

.insight-link:hover {
  text-decoration: underline;
}

.insight-section {
  margin-top: 18px;
}

.section-label {
  margin: 0 0 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--cl-text-muted);
}

.source-bars {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.source-bar-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

.source-bar-count {
  font-size: 11px;
  color: var(--cl-text-muted);
  font-variant-numeric: tabular-nums;
}

.source-bar-track {
  height: 4px;
  border-radius: 999px;
  background: var(--cl-border);
  overflow: hidden;
}

.source-bar-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--el-color-primary);
  min-width: 2px;
}

.empty-topics {
  margin: 0;
  font-size: 12px;
  color: var(--cl-text-muted);
}

.topic-bubbles {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.topic-bubble {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 100%;
  border: 1px solid var(--cl-border);
  border-radius: 999px;
  padding: 3px 10px;
  font-size: 12px;
  background: var(--cl-panel);
  cursor: pointer;
  font-family: inherit;
  color: inherit;
  white-space: nowrap;
}

.topic-bubble:hover {
  border-color: var(--el-color-primary-light-5);
}

.topic-bubble.active {
  border-color: var(--el-color-primary);
  background: rgba(64, 158, 255, 0.14);
  color: var(--el-color-primary);
  font-weight: 600;
}

.topic-bubble.tag {
  border-color: rgba(64, 158, 255, 0.35);
  background: rgba(64, 158, 255, 0.08);
}

.topic-bubble.keyword {
  background: var(--cl-bg);
}

.topic-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.topic-count {
  font-size: 11px;
  color: var(--cl-text-muted);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.topic-bubble.active .topic-count {
  color: var(--el-color-primary);
}

@media (max-width: 1280px) {
  .month-insight {
    display: none;
  }
}
</style>
