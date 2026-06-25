<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { listTimeline, listTimelineMonths } from "../api";
import TimelineMonthInsight from "./TimelineMonthInsight.vue";
import SourceNav from "./SourceNav.vue";
import type { ConversationSummary, SourceCount, TimelineMonthBucket } from "../types";
import { KNOWN_DATA_SOURCES, sourceLabel, sourceAccentColor } from "../utils/dataSource";
import { formatSourceContextSummary } from "../utils/sourceContext";
import {
  type AppLocale,
  formatDayKey,
  formatMonthKey,
  intlLocale,
} from "../utils/locale";
import {
  buildMonthTopics,
  filterConversationsByTopic,
  formatTopicSourceBreakdown,
  isSameTopicFilter,
  sourceCountsFromConversations,
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

const { t, locale } = useI18n();

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
      label: t("filter.allSources"),
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
  return formatMonthKey(key, locale.value as AppLocale);
}

function formatClock(timestamp: number | null) {
  if (!timestamp) return "";
  return new Date(timestamp * 1000).toLocaleString(intlLocale(locale.value as AppLocale), {
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
  return formatDayKey(key, locale.value as AppLocale);
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

function topicFilterSummaryText(conversations: ConversationSummary[]): string | null {
  if (conversations.length === 0) return null;
  const sources = sourceCountsFromConversations(conversations);
  const separator = locale.value === "zh-CN" ? "、" : ", ";
  const fromPart = formatTopicSourceBreakdown(sources, sourceLabel, separator);
  return t("timeline.fromSources", { count: conversations.length, sources: fromPart });
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
      const topicFiltered =
        Boolean(activeTopicFilter.value) && topicFilterMonth.value === key;
      return {
        key,
        label: monthLabel(key),
        conversations: visibleConversations,
        totalConversations: monthConversations.length,
        days: groupDays(visibleConversations),
        count:
          months.value.find((item) => item.month === key)?.conversation_count ??
          monthConversations.length,
        topicFiltered,
        topicFilterSummary: topicFiltered
          ? topicFilterSummaryText(visibleConversations)
          : null,
      };
    })
    .filter((section) => section.totalConversations > 0);

  for (const [key, items] of groups) {
    if (orderedMonths.includes(key)) continue;
    const visibleConversations = conversationsForMonthSection(key, items);
    const topicFiltered = Boolean(activeTopicFilter.value) && topicFilterMonth.value === key;
    sections.push({
      key,
      label: monthLabel(key),
      conversations: visibleConversations,
      totalConversations: items.length,
      days: groupDays(visibleConversations),
      count: items.length,
      topicFiltered,
      topicFilterSummary: topicFiltered ? topicFilterSummaryText(visibleConversations) : null,
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
  if (loadingMore.value) return t("timeline.listFooter.loading");
  if (hasMore.value) {
    const total = visibleTotal.value;
    if (total != null) {
      return t("timeline.listFooter.loadMoreWithTotal", {
        loaded: conversations.value.length,
        total,
      });
    }
    return t("timeline.listFooter.loadMore", { loaded: conversations.value.length });
  }
  const total = visibleTotal.value ?? conversations.value.length;
  return t("timeline.listFooter.total", { total });
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
    <div class="timeline-body">
      <aside class="timeline-sidebar">
        <SourceNav
          :items="sourceNavItems"
          :active-id="filterSource"
          :title="t('filter.sources')"
          show-dots
          @select="setFilterSource"
        />
        <div class="sidebar-divider" />
        <div class="month-nav-title">{{ t("timeline.monthNav") }}</div>
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
              <span class="month-nav-count">{{ t("timeline.monthConversations", { count: bucket.conversation_count }) }}</span>
              <span v-if="bucket.image_count" class="month-nav-images">
                {{ t("timeline.monthImages", { count: bucket.image_count }) }}
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
          :description="t('timeline.empty')"
        />
        <template v-else>
          <section
            v-for="section in groupedSections"
            :key="section.key"
            class="timeline-section"
            :data-timeline-month="section.key"
          >
            <div class="section-header">
              <h3 class="section-title">{{ section.label }}</h3>
              <button
                v-if="section.topicFiltered"
                type="button"
                class="topic-filter-chip"
                @click="clearTopicFilter"
              >
                {{ t("timeline.topicFilter", { label: activeTopicFilter?.label ?? "" }) }}
              </button>
            </div>
            <p v-if="section.topicFilterSummary" class="topic-filter-summary">
              {{ section.topicFilterSummary }}
            </p>
            <p
              v-if="section.topicFiltered && section.conversations.length === 0"
              class="topic-filter-empty"
            >
              {{ t("timeline.noTopicMatch", { label: activeTopicFilter?.label ?? "" }) }}
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
                <span
                  class="item-source-dot"
                  :style="{ background: sourceAccentColor(conversation.source) }"
                  :title="sourceLabel(conversation.source)"
                />
                <span class="col-source">{{ sourceLabel(conversation.source) }}</span>
                <span class="col-title">{{ conversation.title }}</span>
                <span class="col-meta">
                  {{ t("common.messages", { count: conversation.message_count }) }}
                  <template v-if="conversation.model"> · {{ conversation.model }}</template>
                  <template v-if="formatSourceContextSummary(conversation.source_contexts)">
                    · {{ formatSourceContextSummary(conversation.source_contexts) }}
                  </template>
                  <span v-if="conversation.is_starred" class="item-star"> · ★</span>
                </span>
                <span v-if="activityTime(conversation)" class="col-time">
                  {{ formatClock(activityTime(conversation)) }}
                </span>
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
                    {{ t("conversation.latestMessage") }}
                  </button>
                  <button
                    v-if="conversation.has_images"
                    type="button"
                    class="item-action"
                    @click.stop="emit('openGallery', { conversationId: conversation.id })"
                  >
                    {{ t("nav.images") }}
                  </button>
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

.timeline-body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.timeline-sidebar {
  width: var(--cl-sidebar-width);
  flex-shrink: 0;
  border-right: 1px solid var(--cl-border-subtle);
  display: flex;
  flex-direction: column;
  background: var(--cl-panel);
  min-height: 0;
}

.sidebar-divider {
  height: 1px;
  margin: 4px 12px 0;
  background: var(--cl-border-subtle);
}

.month-nav-title {
  padding: 10px 12px 6px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--cl-text-faint);
}

.month-nav-scroll {
  flex: 1;
  overflow: auto;
  padding: 0 8px 12px;
}

.month-nav-item {
  width: 100%;
  border: none;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 7px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--cl-text);
  cursor: pointer;
  text-align: left;
}

.month-nav-item:hover {
  background: var(--cl-hover);
}

.month-nav-item.active {
  background: var(--cl-selected-strong);
  font-weight: 500;
  box-shadow: inset 3px 0 0 var(--cl-accent);
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
  color: var(--cl-text-faint);
  font-variant-numeric: tabular-nums;
}

.month-nav-images {
  font-size: 10px;
  color: var(--cl-text-faint);
  font-variant-numeric: tabular-nums;
}

.timeline-feed {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: 12px 20px 20px;
  position: relative;
  background: var(--cl-panel-elevated);
}

.timeline-feed-inner {
  max-width: var(--cl-content-max-width);
  margin: 0 auto;
}

.timeline-section + .timeline-section {
  margin-top: 28px;
}

.timeline-sidebar :deep(.source-nav-list) {
  padding: 8px 8px 0;
}

.section-header {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 8px 0;
  background: linear-gradient(
    to bottom,
    var(--cl-panel-elevated) 70%,
    rgba(255, 255, 255, 0)
  );
}

.section-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.topic-filter-summary {
  margin: -4px 0 12px;
  font-size: 13px;
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
  margin: 0 0 4px;
  padding: 8px 0 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--cl-text-faint);
  border-bottom: 1px solid var(--cl-border-subtle);
}

.timeline-item {
  width: 100%;
  border: none;
  border-bottom: 1px solid var(--cl-border-subtle);
  background: transparent;
  padding: 0 8px;
  display: grid;
  grid-template-columns: 7px 72px minmax(0, 1fr) minmax(100px, 28%) 52px;
  gap: 8px 12px;
  align-items: center;
  min-height: 48px;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s ease;
}

.timeline-item:hover {
  background: var(--cl-hover);
}

.timeline-item:hover .item-actions {
  opacity: 1;
}

.item-source-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  justify-self: center;
}

.col-source {
  font-size: 11px;
  color: var(--cl-text-faint);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.col-title {
  font-size: 13px;
  font-weight: 500;
  line-height: 1.35;
  color: var(--cl-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.col-meta {
  font-size: 11px;
  color: var(--cl-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.col-time {
  font-size: 12px;
  color: var(--cl-text-faint);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  text-align: right;
}

.item-star {
  color: #c9a227;
}

.item-actions {
  grid-column: 3 / -1;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  opacity: 0;
  transition: opacity 0.12s ease;
  padding-bottom: 4px;
}

.item-action {
  border: none;
  background: transparent;
  padding: 0;
  font-size: 11px;
  color: var(--cl-text-muted);
  cursor: pointer;
  white-space: nowrap;
}

.item-action:hover {
  color: var(--cl-text);
  text-decoration: underline;
}

.timeline-footer {
  padding: 16px 0 8px;
  text-align: center;
  font-size: 12px;
  color: var(--cl-text-muted);
}
</style>
