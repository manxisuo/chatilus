<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readImageDataUrl, listImages, countImages } from "../api";
import ImageLightbox from "./ImageLightbox.vue";
import SourceNav from "./SourceNav.vue";
import InspectorSection from "./InspectorSection.vue";
import type { ImageGalleryItem, SourceCount } from "../types";
import {
  KNOWN_DATA_SOURCES,
  conversationSourceFromId,
  sourceLabel as conversationSourceLabel,
  sourceAccentColor,
} from "../utils/dataSource";
import { galleryItemCacheKey } from "../utils/attachment";
import { type AppLocale, formatDateTime, formatMonthKey } from "../utils/locale";

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

const { t, locale } = useI18n();

const PAGE_SIZE = 60;
const images = ref<ImageGalleryItem[]>([]);
const scopedTotal = ref<number | null>(null);
const loading = ref(false);
const loadingMore = ref(false);
const hasMore = ref(true);
const showUploads = ref(true);
const lightboxVisible = ref(false);
const lightboxIndex = ref(0);
const selectedIndex = ref<number | null>(null);
const imageSrcCache = reactive<Record<string, string>>({});

const selectedImage = computed(() =>
  selectedIndex.value != null ? images.value[selectedIndex.value] ?? null : null,
);

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
      label: t("filter.allImages"),
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
    return t("gallery.stats.breakdown", { generated, upload, other: unknown });
  }
  return t("gallery.stats.pair", { generated, upload });
});

const lightboxImages = computed(() =>
  images.value.map((item) => ({
    path: item.path,
    fileKey: item.file_key,
  })),
);

const lightboxCaptions = computed(() =>
  images.value.map((item) => {
    const source =
      item.source === "generated"
        ? t("gallery.source.generated")
        : item.source === "upload"
          ? t("gallery.source.upload")
          : t("gallery.source.image");
    const prompt = item.prompt?.trim();
    if (prompt) {
      return `${source} · ${item.conversation_title}\n${prompt}`;
    }
    return `${source} · ${item.conversation_title}`;
  }),
);

const footerText = computed(() => {
  if (loadingMore.value) return t("gallery.listFooter.loading");
  if (hasMore.value) {
    const total = visibleTotal.value;
    if (total != null) {
      return t("gallery.listFooter.loadMoreWithTotal", {
        loaded: images.value.length,
        total,
      });
    }
    return t("gallery.listFooter.loadMore", { loaded: images.value.length });
  }
  const total = visibleTotal.value ?? images.value.length;
  return t("gallery.listFooter.total", { total });
});

function imageTypeLabel(source: ImageGalleryItem["source"]) {
  if (source === "generated") return t("gallery.source.generated");
  if (source === "upload") return t("gallery.source.upload");
  return t("gallery.source.other");
}

function selectImage(index: number) {
  selectedIndex.value = index;
}

function formatTime(timestamp: number | null) {
  if (!timestamp) return "";
  return formatDateTime(timestamp, locale.value as AppLocale);
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
      return t("gallery.timeGroup.today");
    case "yesterday":
      return t("gallery.timeGroup.yesterday");
    case "this-week":
      return t("gallery.timeGroup.thisWeek");
    case "unknown":
      return t("gallery.timeGroup.unknown");
    default:
      if (key.startsWith("month:")) {
        return formatMonthKey(key.slice(6), locale.value as AppLocale);
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
  selectedIndex.value = index;
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
    selectedIndex.value = null;
    loadImages(true);
  },
);

onMounted(() => {
  loadImages(true);
});
</script>

<template>
  <div class="image-gallery">
    <aside v-if="!filterConversationId" class="gallery-sidebar">
      <SourceNav
        :items="sourceNavItems"
        :active-id="filterSource"
        :title="t('filter.sources')"
        show-dots
        @select="emit('update:filterSource', $event)"
      />
      <label class="upload-toggle">
        <el-checkbox v-model="showUploads" />
        <span>{{ t("gallery.showUploads") }}</span>
      </label>
      <p v-if="statsText && !filterSource" class="sidebar-stats">{{ statsText }}</p>
    </aside>

    <div class="gallery-main">
      <header
        v-if="filterConversationId || filterMonth"
        class="gallery-context-bar"
      >
        <template v-if="filterConversationId">{{ t("gallery.conversationImages") }}</template>
        <template v-else-if="filterMonth">{{ t("gallery.monthImages", { month: filterMonth }) }}</template>
      </header>

      <el-skeleton v-if="loading" animated :rows="8" class="loading" />

      <el-empty
        v-else-if="images.length === 0"
        class="gallery-empty"
        :description="
          filterSource
            ? t('gallery.emptyFromSource', { source: conversationSourceLabel(filterSource) })
            : showUploads
              ? t('gallery.emptyNoData')
              : t('gallery.emptyGeneratedOnly')
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
              :key="galleryItemCacheKey(item)"
              class="card"
              :class="{ selected: selectedIndex === index, 'card-upload': item.source === 'upload' }"
              @click="selectImage(index)"
            >
              <button
                class="thumb-btn"
                type="button"
                @click.stop="openLightbox(index)"
              >
                <img
                  v-if="imageSrcCache[item.path] !== ''"
                  :src="imageSrc(item.path)"
                  :alt="item.file_key"
                  loading="lazy"
                  @error="onImageError(item.path)"
                />
                <div v-else class="thumb-missing">{{ t("gallery.loadFailed") }}</div>
              </button>
              <div class="card-meta">
                <div class="card-title">{{ item.conversation_title }}</div>
                <div class="card-sub">
                  <span
                    class="source-dot"
                    :style="{ background: sourceAccentColor(conversationSourceFromId(item.conversation_id)) }"
                  />
                  {{ conversationSourceLabel(conversationSourceFromId(item.conversation_id)) }}
                  · {{ formatTime(item.create_time) }}
                </div>
              </div>
            </article>
          </div>
        </section>
        <div class="footer">{{ footerText }}</div>
      </div>
    </div>

    <aside class="gallery-inspector">
      <template v-if="selectedImage">
        <h3 class="inspector-title">{{ t("gallery.inspector.title") }}</h3>
        <div class="inspector-preview">
          <img
            v-if="imageSrcCache[selectedImage.path] !== ''"
            :src="imageSrc(selectedImage.path)"
            :alt="selectedImage.file_key"
            @error="onImageError(selectedImage.path)"
          />
        </div>
        <InspectorSection :title="t('gallery.inspector.sectionBasic')">
          <dl class="cl-inspector-props">
            <div class="cl-inspector-row">
              <dt>{{ t("gallery.inspector.conversation") }}</dt>
              <dd>{{ selectedImage.conversation_title }}</dd>
            </div>
            <div class="cl-inspector-row">
              <dt>{{ t("gallery.inspector.source") }}</dt>
              <dd class="source-value">
                <span
                  class="source-dot"
                  :style="{ background: sourceAccentColor(conversationSourceFromId(selectedImage.conversation_id)) }"
                />
                {{ conversationSourceLabel(conversationSourceFromId(selectedImage.conversation_id)) }}
              </dd>
            </div>
            <div class="cl-inspector-row">
              <dt>{{ t("gallery.inspector.type") }}</dt>
              <dd>{{ imageTypeLabel(selectedImage.source) }}</dd>
            </div>
            <div class="cl-inspector-row">
              <dt>{{ t("gallery.inspector.time") }}</dt>
              <dd>{{ formatTime(selectedImage.create_time) }}</dd>
            </div>
          </dl>
        </InspectorSection>
        <InspectorSection :title="t('gallery.inspector.sectionActions')">
          <div class="cl-inspector-actions">
            <button type="button" class="cl-inspector-action" @click="openLightbox(selectedIndex!)">
              {{ t("gallery.viewFullSize") }}
            </button>
            <button type="button" class="cl-inspector-action" @click="openConversation(selectedImage.conversation_id)">
              {{ t("gallery.openConversation") }}
            </button>
          </div>
        </InspectorSection>
      </template>
      <p v-else class="inspector-empty">{{ t("gallery.inspector.empty") }}</p>
    </aside>

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
  min-height: 0;
  background: var(--cl-panel);
}

.gallery-sidebar {
  width: var(--cl-sidebar-width);
  flex-shrink: 0;
  border-right: 1px solid var(--cl-border-subtle);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 8px 12px;
  background: var(--cl-panel);
  min-height: 0;
}

.upload-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 12px;
  font-size: 12px;
  color: var(--cl-text-muted);
  cursor: pointer;
}

.sidebar-stats {
  margin: 0;
  padding: 0 12px;
  font-size: 11px;
  color: var(--cl-text-faint);
  line-height: 1.4;
}

.gallery-main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--cl-panel-elevated);
}

.gallery-context-bar {
  padding: 10px 20px;
  font-size: 12px;
  color: var(--cl-text-muted);
  border-bottom: 1px solid var(--cl-border-subtle);
}

.gallery-inspector {
  width: var(--cl-insight-width);
  flex-shrink: 0;
  border-left: 1px solid var(--cl-border-subtle);
  padding: 16px;
  overflow: auto;
  background: var(--cl-panel);
}

.inspector-title {
  margin: 0 0 12px;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--cl-text-faint);
}

.inspector-preview {
  margin-bottom: 12px;
  border-radius: 6px;
  overflow: hidden;
  border: 1px solid var(--cl-border-subtle);
  background: var(--cl-bg);
}

.inspector-preview img {
  display: block;
  width: 100%;
  height: auto;
}

.source-value {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.inspector-empty {
  margin: 0;
  font-size: 12px;
  color: var(--cl-text-faint);
  line-height: 1.5;
}

.loading,
.gallery-empty {
  padding: 24px;
  flex: 1;
}

.grid-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}

.time-group {
  padding-top: 4px;
}

.group-title {
  margin: 0;
  padding: 10px 20px 6px;
  font-size: 12px;
  font-weight: 600;
  color: var(--cl-text-faint);
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--cl-panel-elevated);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 12px;
  padding: 4px 20px 16px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  cursor: pointer;
  border-radius: 6px;
  padding: 4px;
  transition: background 0.12s ease;
}

.card:hover {
  background: var(--cl-hover);
}

.card.selected {
  background: var(--cl-selected-strong);
}

.thumb-btn {
  position: relative;
  border: none;
  padding: 0;
  background: var(--cl-bg);
  cursor: zoom-in;
  border-radius: 6px;
  overflow: hidden;
  height: var(--cl-thumb-height);
  width: 100%;
  border: 1px solid var(--cl-border-subtle);
  flex-shrink: 0;
}

.thumb-btn img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
  background: var(--cl-bg);
}

.thumb-missing {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--cl-text-muted);
  background: var(--cl-selected);
}

.card-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  min-height: 34px;
}

.card-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--cl-text);
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-sub {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--cl-text-faint);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.source-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.footer {
  padding: 12px 20px 20px;
  text-align: center;
  font-size: 12px;
  color: var(--cl-text-faint);
}
</style>
