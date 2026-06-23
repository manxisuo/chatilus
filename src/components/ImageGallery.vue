<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readImageDataUrl, listImages, countImages } from "../api";
import ImageLightbox from "./ImageLightbox.vue";
import type { ImageGalleryItem, SourceCount } from "../types";
import {
  KNOWN_DATA_SOURCES,
  conversationSourceFromId,
  sourceLabel as conversationSourceLabel,
  sourceTagType as conversationSourceTagType,
} from "../utils/dataSource";

const props = defineProps<{
  totalCount: number | null;
  generatedCount: number | null;
  uploadCount: number | null;
  filterSource: string | null;
  filterMonth: string | null;
  filterConversationId: string | null;
  imageCountsBySource: SourceCount[];
}>();

const emit = defineEmits<{
  openConversation: [conversationId: string];
  "update:filterSource": [source: string | null];
}>();

const PAGE_SIZE = 60;
const images = ref<ImageGalleryItem[]>([]);
const scopedTotal = ref<number | null>(null);
const loading = ref(false);
const loadingMore = ref(false);
const hasMore = ref(true);
const showUploads = ref(true);
const lightboxVisible = ref(false);
const lightboxIndex = ref(0);
const imageSrcCache = reactive<Record<string, string>>({});

const visibleTotal = computed(() => {
  if (props.filterMonth || props.filterConversationId) {
    return scopedTotal.value;
  }
  if (props.filterSource) {
    const entry = props.imageCountsBySource.find(
      (item) => item.source === props.filterSource,
    );
    return entry?.count ?? null;
  }
  if (showUploads.value) {
    return props.totalCount;
  }
  if (props.generatedCount != null) {
    const unknown = Math.max(
      0,
      (props.totalCount ?? 0) - props.generatedCount - (props.uploadCount ?? 0),
    );
    return props.generatedCount + unknown;
  }
  return props.totalCount;
});

const sourceNavItems = computed(() => {
  const counts = new Map(
    props.imageCountsBySource.map((item) => [item.source, item.count]),
  );
  const items: Array<{ id: string | null; label: string; count: number }> = [
    {
      id: null,
      label: "全部图片",
      count: props.totalCount ?? 0,
    },
  ];

  const seen = new Set<string>();
  for (const source of KNOWN_DATA_SOURCES) {
    seen.add(source);
    items.push({
      id: source,
      label: conversationSourceLabel(source),
      count: counts.get(source) ?? 0,
    });
  }

  for (const entry of props.imageCountsBySource) {
    if (seen.has(entry.source)) {
      continue;
    }
    items.push({
      id: entry.source,
      label: conversationSourceLabel(entry.source),
      count: entry.count,
    });
  }

  return items;
});

const statsText = computed(() => {
  const generated = props.generatedCount;
  const upload = props.uploadCount;
  if (generated == null || upload == null) return "";
  const unknown = Math.max(0, (props.totalCount ?? 0) - generated - upload);
  if (unknown > 0) {
    return `生成 ${generated} · 上传 ${upload} · 其他 ${unknown}`;
  }
  return `生成 ${generated} · 上传 ${upload}`;
});

const lightboxImages = computed(() =>
  images.value.map((item) => ({
    path: item.path,
    fileKey: item.file_key,
  })),
);

const lightboxCaptions = computed(() =>
  images.value.map((item) => {
    const sourceLabel =
      item.source === "generated" ? "生成" : item.source === "upload" ? "上传" : "图片";
    const prompt = item.prompt?.trim();
    if (prompt) {
      return `${sourceLabel} · ${item.conversation_title}\n${prompt}`;
    }
    return `${sourceLabel} · ${item.conversation_title}`;
  }),
);

const footerText = computed(() => {
  if (loadingMore.value) return "加载中…";
  if (hasMore.value) {
    const total = visibleTotal.value;
    if (total != null) {
      return `已加载 ${images.value.length} / ${total}，继续下拉`;
    }
    return `已加载 ${images.value.length} 张，继续下拉`;
  }
  const total = visibleTotal.value ?? images.value.length;
  return `共 ${total} 张图片`;
});

function imageTypeLabel(source: ImageGalleryItem["source"]) {
  if (source === "generated") return "生成";
  if (source === "upload") return "上传";
  return "其他";
}

function imageTypeTagType(source: ImageGalleryItem["source"]) {
  if (source === "generated") return "success";
  if (source === "upload") return "info";
  return "warning";
}

function formatTime(timestamp: number | null) {
  if (!timestamp) return "";
  return new Date(timestamp * 1000).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function timeGroupKey(timestamp: number | null): string {
  if (!timestamp) return "unknown";

  const date = new Date(timestamp * 1000);
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const startOfYesterday = new Date(startOfToday);
  startOfYesterday.setDate(startOfYesterday.getDate() - 1);
  const itemDay = new Date(date.getFullYear(), date.getMonth(), date.getDate());

  if (itemDay.getTime() === startOfToday.getTime()) return "today";
  if (itemDay.getTime() === startOfYesterday.getTime()) return "yesterday";

  const dayOfWeek = now.getDay();
  const mondayOffset = dayOfWeek === 0 ? 6 : dayOfWeek - 1;
  const startOfWeek = new Date(startOfToday);
  startOfWeek.setDate(startOfWeek.getDate() - mondayOffset);

  if (itemDay >= startOfWeek && itemDay < startOfYesterday) return "this-week";

  const month = String(date.getMonth() + 1).padStart(2, "0");
  return `month:${date.getFullYear()}-${month}`;
}

function timeGroupLabel(key: string): string {
  switch (key) {
    case "today":
      return "今天";
    case "yesterday":
      return "昨天";
    case "this-week":
      return "本周";
    case "unknown":
      return "未知时间";
    default:
      if (key.startsWith("month:")) {
        const [year, month] = key.slice(6).split("-");
        return `${year}年${Number(month)}月`;
      }
      return key;
  }
}

function timeGroupSort(key: string): number {
  switch (key) {
    case "today":
      return 0;
    case "yesterday":
      return 1;
    case "this-week":
      return 2;
    case "unknown":
      return 9999;
    default:
      if (key.startsWith("month:")) {
        return 1000 - Number(key.slice(6).replace("-", ""));
      }
      return 5000;
  }
}

const imageGroups = computed(() => {
  const grouped = new Map<
    string,
    Array<{ item: ImageGalleryItem; index: number }>
  >();

  images.value.forEach((item, index) => {
    const key = timeGroupKey(item.create_time);
    const bucket = grouped.get(key) ?? [];
    bucket.push({ item, index });
    grouped.set(key, bucket);
  });

  return [...grouped.entries()]
    .sort(([a], [b]) => timeGroupSort(a) - timeGroupSort(b))
    .map(([key, items]) => ({
      key,
      label: timeGroupLabel(key),
      items,
    }));
});

const lightboxConversationIds = computed(() =>
  images.value.map((item) => item.conversation_id),
);

function imageSrc(path: string) {
  if (!imageSrcCache[path]) {
    imageSrcCache[path] = convertFileSrc(path);
  }
  return imageSrcCache[path];
}

async function onImageError(path: string) {
  if (imageSrcCache[path]?.startsWith("data:")) return;
  try {
    imageSrcCache[path] = await readImageDataUrl(path);
  } catch {
    imageSrcCache[path] = "";
  }
}

async function refreshScopedTotal() {
  if (!props.filterMonth && !props.filterConversationId) {
    scopedTotal.value = null;
    return;
  }
  scopedTotal.value = await countImages(
    showUploads.value,
    props.filterSource,
    props.filterMonth,
    props.filterConversationId,
  );
}

async function loadImages(reset = true) {
  if (reset) {
    if (loading.value) return;
    loading.value = true;
    hasMore.value = true;
  } else {
    if (loading.value || loadingMore.value || !hasMore.value) return;
    loadingMore.value = true;
  }

  try {
    if (reset) {
      await refreshScopedTotal();
    }
    const offset = reset ? 0 : images.value.length;
    const batch = await listImages(
      PAGE_SIZE,
      offset,
      showUploads.value,
      props.filterSource,
      props.filterMonth,
      props.filterConversationId,
    );

    if (reset) {
      images.value = batch;
    } else {
      const existing = new Set(images.value.map((item) => `${item.message_id}:${item.path}`));
      images.value = [
        ...images.value,
        ...batch.filter((item) => !existing.has(`${item.message_id}:${item.path}`)),
      ];
    }

    if (scopedTotal.value != null) {
      hasMore.value = images.value.length < scopedTotal.value;
    } else {
      hasMore.value = batch.length === PAGE_SIZE;
    }
  } finally {
    loading.value = false;
    loadingMore.value = false;
  }
}

function onScroll(event: Event) {
  if (!hasMore.value || loading.value || loadingMore.value) return;
  const el = event.target as HTMLElement;
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 160) {
    loadImages(false);
  }
}

function openLightbox(index: number) {
  lightboxIndex.value = index;
  lightboxVisible.value = true;
}

function openConversation(conversationId: string) {
  emit("openConversation", conversationId);
}

watch(showUploads, () => {
  loadImages(true);
});

watch(
  () => [props.filterSource, props.filterMonth, props.filterConversationId] as const,
  () => {
    loadImages(true);
  },
);

onMounted(() => {
  loadImages(true);
});
</script>

<template>
  <div class="image-gallery">
    <header class="gallery-header">
      <div class="gallery-header-main">
        <p class="subtitle">
          <template v-if="filterConversationId">当前对话的图片</template>
          <template v-else-if="filterMonth">{{ filterMonth }} 的图片</template>
          <template v-else>浏览所有对话中的图片，点击放大，或跳回所属对话</template>
        </p>
        <p v-if="statsText && !filterSource" class="stats">{{ statsText }}</p>
        <div class="source-nav">
          <button
            v-for="item in sourceNavItems"
            :key="item.id ?? 'all'"
            type="button"
            class="source-nav-item"
            :class="{ active: filterSource === item.id }"
            @click="emit('update:filterSource', item.id)"
          >
            <span>{{ item.label }}</span>
            <span class="source-nav-count">{{ item.count }}</span>
          </button>
        </div>
      </div>
      <el-checkbox v-model="showUploads" label="显示用户上传的图片" />
    </header>

    <el-skeleton v-if="loading" animated :rows="8" class="loading" />

    <el-empty
      v-else-if="images.length === 0"
      :description="
        filterSource
          ? `暂无来自 ${conversationSourceLabel(filterSource)} 的图片`
          : showUploads
            ? '暂无图片，请先导入包含图片的对话数据'
            : '暂无生成图片，可勾选显示用户上传的图片'
      "
    />

    <div v-else class="grid-scroll" @scroll.passive="onScroll">
      <section
        v-for="group in imageGroups"
        :key="group.key"
        class="time-group"
      >
        <h3 class="group-title">{{ group.label }}</h3>
        <div class="grid">
          <article
            v-for="{ item, index } in group.items"
            :key="`${item.message_id}:${item.path}`"
            class="card"
            :class="{ 'card-upload': item.source === 'upload' }"
          >
            <button class="thumb-btn" type="button" @click="openLightbox(index)">
              <img
                v-if="imageSrcCache[item.path] !== ''"
                :src="imageSrc(item.path)"
                :alt="item.file_key"
                loading="lazy"
                @error="onImageError(item.path)"
              />
              <div v-else class="thumb-missing">无法加载</div>
              <span class="source-badge" :class="`source-${item.source}`">
                {{ imageTypeLabel(item.source) }}
              </span>
            </button>
            <div class="card-meta">
              <button
                class="conv-link"
                type="button"
                title="打开所属对话"
                @click="openConversation(item.conversation_id)"
              >
                {{ item.conversation_title }}
              </button>
              <div class="meta-row">
                <div class="meta-tags">
                  <el-tag
                    v-if="!filterSource"
                    size="small"
                    :type="conversationSourceTagType(conversationSourceFromId(item.conversation_id))"
                    effect="plain"
                  >
                    {{ conversationSourceLabel(conversationSourceFromId(item.conversation_id)) }}
                  </el-tag>
                  <el-tag size="small" :type="imageTypeTagType(item.source)" effect="plain">
                    {{ imageTypeLabel(item.source) }}
                  </el-tag>
                </div>
                <span class="time">{{ formatTime(item.create_time) }}</span>
              </div>
            </div>
          </article>
        </div>
      </section>
      <div class="footer">{{ footerText }}</div>
    </div>

    <ImageLightbox
      v-model:visible="lightboxVisible"
      :images="lightboxImages"
      :initial-index="lightboxIndex"
      :resolve-src="imageSrc"
      :on-image-error="onImageError"
      :captions="lightboxCaptions"
      :conversation-ids="lightboxConversationIds"
      @open-conversation="openConversation"
    />
  </div>
</template>

<style scoped>
.image-gallery {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--cl-panel);
}

.gallery-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 20px 24px 12px;
  border-bottom: 1px solid var(--cl-border);
}

.gallery-header-main {
  flex: 1;
  min-width: 0;
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

.subtitle {
  margin: 0;
  font-size: 13px;
  color: var(--cl-text-muted);
}

.stats {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--cl-text-muted);
}

.loading {
  padding: 24px;
}

.grid-scroll {
  flex: 1;
  overflow: auto;
  min-height: 0;
}

.time-group {
  padding-top: 8px;
}

.time-group:first-child {
  padding-top: 0;
}

.group-title {
  margin: 0;
  padding: 12px 24px 4px;
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text-muted);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 16px;
  padding: 8px 24px 16px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.thumb-btn {
  position: relative;
  border: none;
  padding: 0;
  background: transparent;
  cursor: zoom-in;
  border-radius: 10px;
  overflow: hidden;
  aspect-ratio: 1;
  border: 1px solid var(--cl-border);
}

.card-upload .thumb-btn {
  border-color: rgba(64, 158, 255, 0.45);
}

.thumb-btn img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.source-badge {
  position: absolute;
  top: 8px;
  left: 8px;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  color: #fff;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(4px);
}

.source-generated {
  background: rgba(103, 194, 58, 0.85);
}

.source-upload {
  background: rgba(64, 158, 255, 0.85);
}

.source-unknown {
  background: rgba(230, 162, 60, 0.85);
}

.thumb-missing {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--cl-text-muted);
  background: rgba(127, 127, 127, 0.08);
}

.card-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.meta-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.meta-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  min-width: 0;
}

.conv-link {
  border: none;
  background: transparent;
  padding: 0;
  text-align: left;
  font-size: 12px;
  font-weight: 600;
  color: var(--el-color-primary);
  cursor: pointer;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.conv-link:hover {
  text-decoration: underline;
}

.time {
  font-size: 11px;
  color: var(--cl-text-muted);
  white-space: nowrap;
}

.footer {
  padding: 8px 24px 24px;
  text-align: center;
  font-size: 12px;
  color: var(--cl-text-muted);
}
</style>
