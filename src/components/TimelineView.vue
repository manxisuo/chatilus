<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listTimeline, listTimelineMonths } from "../api";
import TimelineMonthInsight from "./TimelineMonthInsight.vue";
import type { ConversationSummary, SourceCount, TimelineMonthBucket } from "../types";
import { KNOWN_DATA_SOURCES, sourceLabel, sourceTagType } from "../utils/dataSource";
import {
  buildMonthTopics,
  filterConversationsByTopic,
  isSameTopicFilter,
  type TopicFilter,
} from "../utils/timelineTopics";

const props = defineProps<{
  totalCount: number | null;
  conversationCountsBySource: SourceCount[];
  filterSource: string | null;
}>();

const emit = defineEmits<{
  openConversation: [conversationId: string];
  openMessage: [conversationId: string, messageId: string];
  openGallery: [options: { month?: string; conversationId?: string }];
  "update:filterSource": [source: string | null];
}>();

const PAGE_SIZE = 80;
const months = ref<TimelineMonthBucket[]>([]);
const conversations = ref<ConversationSummary[]>([]);
const loading = ref(false);
const loadingMore = ref(false);
const hasMore = ref(true);
const activeMonth = ref<string | null>(null);
const feedRef = ref<HTMLElement | null>(null);
const monthNavRef = ref<HTMLElement | null>(null);
const jumpingToMonth = ref(false);
const monthInsightLoading = ref(false);
const activeTopicFilter = ref<TopicFilter | null>(null);
const topicFilterMonth = ref<string | null>(null);
let monthPreloadTimer: ReturnType<typeof setTimeout> | null = null;

const FEED_PADDING_TOP = 16;

const visibleTotal = computed(() => {
  if (props.filterSource) {
    const entry = props.conversationCountsBySource.find(
      (item) => item.source === props.filterSource,
    );
    return entry?.count ?? null;
  }
  return props.totalCount;
});

const sourceNavItems = computed(() => {
  const counts = new Map(
    props.conversationCountsBySource.map((item) => [item.source, item.count]),
  );
  const items: Array<{ id: string | null; label: string; count: number }> = [
    {
      id: null,
      label: "全部来源",
      count: props.totalCount ?? 0,
    },
  ];

  const seen = new Set<string>();
  for (const source of KNOWN_DATA_SOURCES) {
    seen.add(source);
    items.push({
      id: source,
      label: sourceLabel(source),
      count: counts.get(source) ?? 0,
    });
  }

  for (const entry of props.conversationCountsBySource) {
    if (seen.has(entry.source)) continue;
    items.push({
      id: entry.source,
      label: sourceLabel(entry.source),
      count: entry.count,
    });
  }

  return items;
});

function activityTime(conversation: ConversationSummary): number | null {
  return conversation.update_time ?? conversation.create_time;
}

function monthKey(timestamp: number | null): string | null {
  if (!timestamp) return null;
  const date = new Date(timestamp * 1000);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  return `${date.getFullYear()}-${month}`;
}

function monthLabel(key: string): string {
  const [year, month] = key.split("-");
  return `${year}年${Number(month)}月`;
}

function formatClock(timestamp: number | null) {
  if (!timestamp) return "";
  return new Date(timestamp * 1000).toLocaleString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function dayKey(timestamp: number | null): string | null {
  if (!timestamp) return null;
  const date = new Date(timestamp * 1000);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function dayLabel(key: string): string {
  const [, month, day] = key.split("-");
  return `${Number(month)}月${Number(day)}日`;
}

function conversationMonth(conversation: ConversationSummary): string | null {
  return conversation.activity_month ?? monthKey(activityTime(conversation));
}

function groupDays(items: ConversationSummary[]) {
  const groups = new Map<string, ConversationSummary[]>();
  for (const conversation of items) {
    const key = dayKey(activityTime(conversation));
    if (!key) continue;
    const bucket = groups.get(key) ?? [];
    bucket.push(conversation);
    groups.set(key, bucket);
  }

  return [...groups.entries()]
    .sort(([left], [right]) => right.localeCompare(left))
    .map(([key, dayConversations]) => ({
      key,
      label: dayLabel(key),
      conversations: dayConversations.sort(
        (left, right) => (activityTime(right) ?? 0) - (activityTime(left) ?? 0),
      ),
    }));
}

function sourceCountsForMonth(monthKey: string, items: ConversationSummary[]): SourceCount[] {
  const fromBackend = months.value.find((item) => item.month === monthKey)?.source_counts;
  if (fromBackend && fromBackend.length > 0) {
    return fromBackend;
  }

  const tallies = new Map<string, number>();
  for (const conversation of items) {
    const source = conversation.source ?? "chatgpt";
    tallies.set(source, (tallies.get(source) ?? 0) + 1);
  }
  return [...tallies.entries()]
    .map(([source, count]) => ({ source, count }))
    .sort((left, right) => right.count - left.count || left.source.localeCompare(right.source));
}

function mergeConversations(batch: ConversationSummary[]) {
  const byId = new Map(conversations.value.map((item) => [item.id, item]));
  for (const item of batch) {
    byId.set(item.id, item);
  }
  conversations.value = [...byId.values()].sort(
    (left, right) => (activityTime(right) ?? 0) - (activityTime(left) ?? 0),
  );
}

function conversationsForMonthSection(monthKey: string, items: ConversationSummary[]) {
  if (!activeTopicFilter.value || topicFilterMonth.value !== monthKey) {
    return items;
  }
  return filterConversationsByTopic(items, activeTopicFilter.value);
}

const groupedSections = computed(() => {
  const groups = new Map<string, ConversationSummary[]>();
  for (const conversation of conversations.value) {
    const key = conversationMonth(conversation);
    if (!key) continue;
    const bucket = groups.get(key) ?? [];
    bucket.push(conversation);
    groups.set(key, bucket);
  }

  const orderedMonths =
    months.value.length > 0
      ? months.value.map((item) => item.month)
      : [...groups.keys()].sort((left, right) => right.localeCompare(left));

  const sections = orderedMonths
    .map((key) => {
      const monthConversations = groups.get(key) ?? [];
      const visibleConversations = conversationsForMonthSection(key, monthConversations);
      return {
        key,
        label: monthLabel(key),
        conversations: visibleConversations,
        totalConversations: monthConversations.length,
        days: groupDays(visibleConversations),
        count:
          months.value.find((item) => item.month === key)?.conversation_count ??
          monthConversations.length,
        imageCount:
          months.value.find((item) => item.month === key)?.image_count ?? 0,
        sourceCounts: sourceCountsForMonth(key, monthConversations),
        topicFiltered:
          Boolean(activeTopicFilter.value) && topicFilterMonth.value === key,
      };
    })
    .filter((section) => section.totalConversations > 0);

  for (const [key, items] of groups) {
    if (orderedMonths.includes(key)) continue;
    const visibleConversations = conversationsForMonthSection(key, items);
    sections.push({
      key,
      label: monthLabel(key),
      conversations: visibleConversations,
      totalConversations: items.length,
      days: groupDays(visibleConversations),
      count: items.length,
      imageCount: 0,
      sourceCounts: sourceCountsForMonth(key, items),
      topicFiltered: Boolean(activeTopicFilter.value) && topicFilterMonth.value === key,
    });
  }

  return sections.sort((left, right) => right.key.localeCompare(left.key));
});

const activeMonthBucket = computed(
  () => months.value.find((item) => item.month === activeMonth.value) ?? null,
);

const activeMonthConversations = computed(() => {
  if (!activeMonth.value) return [];
  return conversations.value.filter(
    (conversation) => conversationMonth(conversation) === activeMonth.value,
  );
});

const activeMonthInsight = computed(() => {
  const month = activeMonth.value;
  const items = activeMonthConversations.value;
  const bucket = activeMonthBucket.value;
  const days = new Set(
    items.map((conversation) => dayKey(activityTime(conversation))).filter(Boolean),
  );

  return {
    monthLabel: month ? monthLabel(month) : null,
    loadedCount: items.length,
    totalCount: bucket?.conversation_count ?? items.length,
    imageCount: bucket?.image_count ?? 0,
    messageCount: items.reduce((sum, conversation) => sum + conversation.message_count, 0),
    starredCount: items.filter((conversation) => conversation.is_starred).length,
    activeDays: days.size,
    sourceCounts: month ? sourceCountsForMonth(month, items) : [],
    topics: buildMonthTopics(items),
  };
});

const footerText = computed(() => {
  if (loadingMore.value) return "加载中…";
  if (hasMore.value) {
    const total = visibleTotal.value;
    if (total != null) {
      return `已加载 ${conversations.value.length} / ${total}，继续下拉`;
    }
    return `已加载 ${conversations.value.length} 条，继续下拉`;
  }
  const total = visibleTotal.value ?? conversations.value.length;
  return `共 ${total} 段对话`;
});

async function loadMonths() {
  months.value = await listTimelineMonths(props.filterSource);
  if (!activeMonth.value && months.value.length > 0) {
    activeMonth.value = months.value[0].month;
  }
}

async function loadTimeline(reset = true) {
  if (reset) {
    if (loading.value) return;
    loading.value = true;
    conversations.value = [];
    hasMore.value = true;
  } else {
    if (loadingMore.value || !hasMore.value) return;
    loadingMore.value = true;
  }

  try {
    const offset = reset ? 0 : conversations.value.length;
    const batch = await listTimeline({
      source: props.filterSource,
      limit: PAGE_SIZE,
      offset,
    });
    if (reset) {
      conversations.value = batch;
    } else {
      mergeConversations(batch);
    }
    hasMore.value = batch.length >= PAGE_SIZE;
  } finally {
    loading.value = false;
    loadingMore.value = false;
  }
}

async function reload() {
  await Promise.all([loadMonths(), loadTimeline(true)]);
}

function onFeedScroll(event: Event) {
  const el = event.target as HTMLElement;
  if (!jumpingToMonth.value) {
    updateActiveMonthFromScroll(el);
  }

  if (!hasMore.value || loading.value || loadingMore.value) return;
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 160) {
    void loadTimeline(false);
  }
}

function updateActiveMonthFromScroll(container: HTMLElement) {
  const sections = container.querySelectorAll<HTMLElement>("[data-timeline-month]");
  const containerTop = container.getBoundingClientRect().top;
  const anchor = 72;
  let current: string | null = null;
  let closestTop = -Infinity;

  for (const section of sections) {
    const header = section.querySelector<HTMLElement>(".section-header");
    if (!header) continue;
    const headerTop = header.getBoundingClientRect().top - containerTop;
    if (headerTop <= anchor && headerTop > closestTop) {
      closestTop = headerTop;
      current = section.dataset.timelineMonth ?? null;
    }
  }

  if (current && current !== activeMonth.value) {
    activeMonth.value = current;
    scrollMonthNavIntoView(current);
  }
}

function scrollMonthNavIntoView(month: string) {
  const nav = monthNavRef.value;
  const button = nav?.querySelector<HTMLElement>(`[data-month-nav="${month}"]`);
  button?.scrollIntoView({ block: "nearest" });
}

async function ensureMonthLoaded(month: string) {
  const batch = await listTimeline({
    source: props.filterSource,
    month,
    limit: 500,
  });
  mergeConversations(batch);
}

function elementScrollTop(container: HTMLElement, element: HTMLElement): number {
  return (
    element.getBoundingClientRect().top -
    container.getBoundingClientRect().top +
    container.scrollTop
  );
}

function scrollFeedToMonth(container: HTMLElement, month: string) {
  const section = container.querySelector<HTMLElement>(`[data-timeline-month="${month}"]`);
  if (!section) return;
  container.scrollTop = Math.max(0, elementScrollTop(container, section) - FEED_PADDING_TOP);
}

async function jumpToMonth(month: string) {
  clearTopicFilter();
  jumpingToMonth.value = true;
  activeMonth.value = month;
  try {
    await ensureMonthLoaded(month);
    await nextTick();
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );

    const container = feedRef.value;
    if (!container) return;
    scrollFeedToMonth(container, month);
    scrollMonthNavIntoView(month);
    requestAnimationFrame(() => {
      scrollFeedToMonth(container, month);
    });
  } finally {
    requestAnimationFrame(() => {
      jumpingToMonth.value = false;
    });
  }
}

function setFilterSource(source: string | null) {
  emit("update:filterSource", source);
}

function toggleTopicFilter(topic: TopicFilter) {
  if (isSameTopicFilter(activeTopicFilter.value, topic)) {
    clearTopicFilter();
    return;
  }

  const month = activeMonth.value;
  if (!month) return;

  activeTopicFilter.value = topic;
  topicFilterMonth.value = month;
  void repositionFeedForTopicFilter(month);
}

async function repositionFeedForTopicFilter(month: string) {
  jumpingToMonth.value = true;
  try {
    await ensureMonthLoaded(month);
    await nextTick();
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );

    const container = feedRef.value;
    if (!container) return;
    scrollFeedToMonth(container, month);
    activeMonth.value = month;
    scrollMonthNavIntoView(month);
    requestAnimationFrame(() => {
      scrollFeedToMonth(container, month);
    });
  } finally {
    requestAnimationFrame(() => {
      jumpingToMonth.value = false;
    });
  }
}

function clearTopicFilter() {
  activeTopicFilter.value = null;
  topicFilterMonth.value = null;
}

watch(
  () => props.filterSource,
  () => {
    activeMonth.value = null;
    activeTopicFilter.value = null;
    topicFilterMonth.value = null;
    void reload();
  },
);

watch(activeMonth, (month, previousMonth) => {
  if (month !== previousMonth && !jumpingToMonth.value) {
    activeTopicFilter.value = null;
    topicFilterMonth.value = null;
  }
  if (!month || jumpingToMonth.value) return;
  if (monthPreloadTimer) clearTimeout(monthPreloadTimer);
  monthPreloadTimer = setTimeout(() => {
    const loaded = activeMonthConversations.value.length;
    const total = activeMonthBucket.value?.conversation_count ?? loaded;
    if (loaded >= total) return;

    monthInsightLoading.value = true;
    void ensureMonthLoaded(month).finally(() => {
      monthInsightLoading.value = false;
    });
  }, 300);
});

function openActiveMonthGallery() {
  if (!activeMonth.value) return;
  emit("openGallery", { month: activeMonth.value });
}

onMounted(() => {
  void reload();
});

onUnmounted(() => {
  if (monthPreloadTimer) clearTimeout(monthPreloadTimer);
});
</script>

<template>
  <div class="timeline-view">
    <header class="timeline-header">
      <div class="timeline-header-main">
        <h2 class="title">时间线</h2>
        <p class="subtitle">按月份回顾你与 AI 的对话足迹，跨 ChatGPT、Cursor、Gemini 等来源混排。</p>
        <div class="source-nav">
          <button
            v-for="item in sourceNavItems"
            :key="item.id ?? 'all'"
            type="button"
            class="source-nav-item"
            :class="{ active: filterSource === item.id }"
            @click="setFilterSource(item.id)"
          >
            <span>{{ item.label }}</span>
            <span class="source-nav-count">{{ item.count.toLocaleString() }}</span>
          </button>
        </div>
      </div>
    </header>

    <div class="timeline-body">
      <aside class="month-nav">
        <div class="month-nav-title">月份</div>
        <div ref="monthNavRef" class="month-nav-scroll">
          <el-skeleton v-if="loading && months.length === 0" animated :rows="8" />
          <button
            v-for="bucket in months"
            :key="bucket.month"
            type="button"
            class="month-nav-item"
            :class="{ active: activeMonth === bucket.month }"
            :data-month-nav="bucket.month"
            @click="jumpToMonth(bucket.month)"
          >
            <span class="month-nav-label">{{ monthLabel(bucket.month) }}</span>
            <span class="month-nav-meta">
              <span class="month-nav-count">{{ bucket.conversation_count }}</span>
              <span v-if="bucket.image_count" class="month-nav-images">
                {{ bucket.image_count }} 图
              </span>
            </span>
          </button>
        </div>
      </aside>

      <div ref="feedRef" class="timeline-feed" @scroll.passive="onFeedScroll">
        <div class="timeline-feed-inner">
        <el-skeleton v-if="loading" animated :rows="10" />
        <el-empty
          v-else-if="conversations.length === 0"
          description="暂无带时间戳的对话，导入数据后即可按月份浏览"
        />
        <template v-else>
          <section
            v-for="section in groupedSections"
            :key="section.key"
            class="timeline-section"
            :data-timeline-month="section.key"
          >
            <div class="section-header">
              <div class="section-header-main">
                <h3 class="section-title">{{ section.label }}</h3>
                <div v-if="section.sourceCounts.length" class="section-sources">
                  <span
                    v-for="item in section.sourceCounts"
                    :key="item.source"
                    class="section-source-pill"
                  >
                    {{ sourceLabel(item.source) }} {{ item.count }}
                  </span>
                </div>
              </div>
              <div class="section-header-actions">
                <button
                  v-if="section.topicFiltered"
                  type="button"
                  class="topic-filter-chip"
                  @click="clearTopicFilter"
                >
                  话题：{{ activeTopicFilter?.label }} ✕
                </button>
                <span class="section-count">
                  <template v-if="section.topicFiltered">
                    {{ section.conversations.length }} / {{ section.totalConversations }}
                  </template>
                  <template v-else>
                    {{ section.conversations.length
                    }}<template v-if="section.count > section.conversations.length">
                      / {{ section.count }}</template
                    >
                  </template>
                  条对话
                </span>
                <button
                  v-if="section.imageCount > 0"
                  type="button"
                  class="section-link"
                  @click="emit('openGallery', { month: section.key })"
                >
                  {{ section.imageCount }} 张 · 本月图片
                </button>
              </div>
            </div>
            <p
              v-if="section.topicFiltered && section.conversations.length === 0"
              class="topic-filter-empty"
            >
              本月没有匹配「{{ activeTopicFilter?.label }}」的对话
            </p>
            <div
              v-for="day in section.days"
              :key="day.key"
              class="timeline-day"
            >
              <h4 class="day-header">{{ day.label }}</h4>
              <button
                v-for="conversation in day.conversations"
                :key="conversation.id"
                type="button"
                class="timeline-item"
                @click="emit('openConversation', conversation.id)"
              >
                <div class="item-top">
                  <el-tag
                    size="small"
                    :type="sourceTagType(conversation.source)"
                    effect="plain"
                  >
                    {{ sourceLabel(conversation.source) }}
                  </el-tag>
                  <span v-if="activityTime(conversation)" class="item-time">
                    {{ formatClock(activityTime(conversation)) }}
                  </span>
                </div>
                <div class="item-title">{{ conversation.title }}</div>
                <div class="item-meta">
                  <span>{{ conversation.message_count }} 条消息</span>
                  <span v-if="conversation.model">{{ conversation.model }}</span>
                  <span v-if="conversation.is_starred" class="item-star">★ 已收藏</span>
                </div>
                <div
                  v-if="conversation.latest_message_id || conversation.has_images"
                  class="item-actions"
                >
                  <button
                    v-if="conversation.latest_message_id"
                    type="button"
                    class="item-action"
                    @click.stop="
                      emit('openMessage', conversation.id, conversation.latest_message_id!)
                    "
                  >
                    最新消息
                  </button>
                  <button
                    v-if="conversation.has_images"
                    type="button"
                    class="item-action"
                    @click.stop="emit('openGallery', { conversationId: conversation.id })"
                  >
                    图片
                  </button>
                </div>
                <div v-if="conversation.tags.length" class="item-tags">
                  <el-tag
                    v-for="tag in conversation.tags"
                    :key="tag"
                    size="small"
                    type="info"
                    effect="plain"
                  >
                    {{ tag }}
                  </el-tag>
                </div>
              </button>
            </div>
          </section>
          <div class="timeline-footer">{{ footerText }}</div>
        </template>
        </div>
      </div>

      <TimelineMonthInsight
        :month-label="activeMonthInsight.monthLabel"
        :loaded-count="activeMonthInsight.loadedCount"
        :total-count="activeMonthInsight.totalCount"
        :image-count="activeMonthInsight.imageCount"
        :message-count="activeMonthInsight.messageCount"
        :starred-count="activeMonthInsight.starredCount"
        :active-days="activeMonthInsight.activeDays"
        :source-counts="activeMonthInsight.sourceCounts"
        :topics="activeMonthInsight.topics"
        :loading="monthInsightLoading"
        :active-topic-filter="activeTopicFilter"
        @open-gallery="openActiveMonthGallery"
        @select-topic="toggleTopicFilter"
      />
    </div>
  </div>
</template>

<style scoped>
.timeline-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--cl-panel);
}

.timeline-header {
  padding: 20px 24px 12px;
  border-bottom: 1px solid var(--cl-border);
}

.timeline-header-main {
  min-width: 0;
}

.title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
}

.subtitle {
  margin: 6px 0 0;
  font-size: 13px;
  color: var(--cl-text-muted);
}

.source-nav {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 12px;
}

.source-nav-item {
  border: 1px solid var(--cl-border);
  background: var(--cl-bg);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  font-size: 12px;
  color: var(--cl-text);
  cursor: pointer;
}

.source-nav-item:hover {
  border-color: var(--el-color-primary-light-5);
}

.source-nav-item.active {
  border-color: var(--el-color-primary);
  background: rgba(64, 158, 255, 0.1);
  color: var(--el-color-primary);
  font-weight: 600;
}

.source-nav-count {
  font-size: 11px;
  color: var(--cl-text-muted);
  font-variant-numeric: tabular-nums;
}

.source-nav-item.active .source-nav-count {
  color: var(--el-color-primary);
}

.timeline-body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.month-nav {
  width: var(--cl-sidebar-width);
  flex-shrink: 0;
  border-right: 1px solid var(--cl-border);
  display: flex;
  flex-direction: column;
  background: var(--cl-panel);
}

.month-nav-title {
  padding: 14px 16px 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--cl-text-muted);
}

.month-nav-scroll {
  flex: 1;
  overflow: auto;
  padding: 0 12px 12px;
}

.month-nav-item {
  width: 100%;
  border: none;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 8px;
  font-size: 13px;
  color: var(--cl-text);
  cursor: pointer;
  text-align: left;
}

.month-nav-item:hover {
  background: rgba(64, 158, 255, 0.08);
}

.month-nav-item.active {
  background: rgba(64, 158, 255, 0.14);
  color: var(--el-color-primary);
  font-weight: 600;
}

.month-nav-label {
  min-width: 0;
  flex: 1;
}

.month-nav-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  flex-shrink: 0;
}

.month-nav-count {
  font-size: 11px;
  color: var(--cl-text-muted);
  font-variant-numeric: tabular-nums;
}

.month-nav-images {
  font-size: 10px;
  color: var(--cl-text-muted);
  font-variant-numeric: tabular-nums;
}

.month-nav-item.active .month-nav-count,
.month-nav-item.active .month-nav-images {
  color: var(--el-color-primary);
}

.timeline-feed {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: 16px 24px 24px;
  position: relative;
}

.timeline-feed-inner {
  max-width: var(--cl-content-max-width);
  margin: 0 auto;
}

.timeline-section + .timeline-section {
  margin-top: 28px;
}

.section-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 12px;
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 8px 0;
  background: linear-gradient(
    to bottom,
    var(--cl-panel) 70%,
    rgba(255, 255, 255, 0)
  );
}

.section-header-main {
  min-width: 0;
}

.section-sources {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 6px;
}

.section-source-pill {
  font-size: 11px;
  color: var(--cl-text-muted);
  border: 1px solid var(--cl-border);
  border-radius: 999px;
  padding: 1px 8px;
  background: var(--cl-bg);
}

.section-header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.section-link {
  border: none;
  background: transparent;
  padding: 0;
  font-size: 12px;
  color: var(--el-color-primary);
  cursor: pointer;
}

.section-link:hover {
  text-decoration: underline;
}

.section-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.section-count {
  font-size: 12px;
  color: var(--cl-text-muted);
}

.topic-filter-chip {
  border: 1px solid var(--el-color-primary);
  background: rgba(64, 158, 255, 0.1);
  border-radius: 999px;
  padding: 2px 10px;
  font-size: 12px;
  color: var(--el-color-primary);
  cursor: pointer;
}

.topic-filter-chip:hover {
  background: rgba(64, 158, 255, 0.16);
}

.topic-filter-empty {
  margin: 0 0 12px;
  font-size: 13px;
  color: var(--cl-text-muted);
}

.timeline-day + .timeline-day {
  margin-top: 18px;
}

.day-header {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text-muted);
}

.timeline-item {
  width: 100%;
  border: 1px solid var(--cl-border);
  background: var(--cl-bg);
  border-radius: 12px;
  padding: 14px 16px;
  margin-bottom: 10px;
  text-align: left;
  cursor: pointer;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.timeline-item:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.04);
}

.item-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.item-time {
  font-size: 12px;
  color: var(--cl-text-muted);
}

.item-title {
  margin-top: 8px;
  font-size: 15px;
  font-weight: 600;
  line-height: 1.45;
}

.item-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin-top: 8px;
  font-size: 12px;
  line-height: 1.4;
  color: var(--cl-text-muted);
}

.item-meta > span {
  line-height: 1.4;
}

.item-star {
  color: #e6a23c;
}

.item-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}

.item-action {
  border: 1px solid var(--cl-border);
  background: var(--cl-panel);
  border-radius: 999px;
  padding: 2px 10px;
  font-size: 12px;
  color: var(--cl-text-muted);
  cursor: pointer;
}

.item-action:hover {
  border-color: var(--el-color-primary-light-5);
  color: var(--el-color-primary);
}

.item-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 10px;
}

.timeline-footer {
  padding: 16px 0 8px;
  text-align: center;
  font-size: 12px;
  color: var(--cl-text-muted);
}
</style>
