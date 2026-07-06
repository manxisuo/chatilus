<script setup lang="ts">
import { computed, nextTick, onUnmounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readImageDataUrl } from "../api";
import { renderMarkdown } from "../utils/markdown";
import { attachmentCacheKey } from "../utils/attachment";
import { type AppLocale, formatDateTime } from "../utils/locale";
import ImageLightbox from "./ImageLightbox.vue";
import type { MessageView as MessageItem } from "../types";

const props = defineProps<{
  messages: MessageItem[];
  title: string;
  loading: boolean;
  conversationStarred: boolean;
  conversationTags: string[];
  dataSource?: string | null;
  highlightMessageId?: string | null;
  showInfoButton?: boolean;
}>();

const emit = defineEmits<{
  toggleConversationStar: [];
  exportMarkdown: [];
  toggleMessageStar: [messageId: string, starred: boolean];
  openConversationInfo: [];
}>();

const { t, locale } = useI18n();

const imageSrcCache = reactive<Record<string, string>>({});
const lightboxVisible = ref(false);
const lightboxIndex = ref(0);
const messagesContainerRef = ref<HTMLElement | null>(null);
const highlightPulseKey = ref(0);

let scrollSession = 0;
let resizeObserver: ResizeObserver | null = null;
let resizeTimer: ReturnType<typeof setTimeout> | null = null;
let scrollWatchTimer: ReturnType<typeof setTimeout> | null = null;

function stopScrollWatch() {
  resizeObserver?.disconnect();
  resizeObserver = null;
  if (resizeTimer) {
    clearTimeout(resizeTimer);
    resizeTimer = null;
  }
  if (scrollWatchTimer) {
    clearTimeout(scrollWatchTimer);
    scrollWatchTimer = null;
  }
}

onUnmounted(stopScrollWatch);

watch(
  () => props.messages,
  () => {
    lightboxVisible.value = false;
    stopScrollWatch();
  },
);

function layoutAffectingImages(
  container: HTMLElement,
  target: HTMLElement,
): HTMLImageElement[] {
  const images: HTMLImageElement[] = [];
  for (const message of container.querySelectorAll("article.message")) {
    images.push(...message.querySelectorAll<HTMLImageElement>("img"));
    if (message === target) break;
  }
  return images;
}

function waitForImages(images: HTMLImageElement[]): Promise<void> {
  const pending = images.filter((image) => !image.complete);
  if (pending.length === 0) return Promise.resolve();
  return new Promise((resolve) => {
    let remaining = pending.length;
    const done = () => {
      remaining -= 1;
      if (remaining === 0) resolve();
    };
    for (const image of pending) {
      image.addEventListener("load", done, { once: true });
      image.addEventListener("error", done, { once: true });
    }
  });
}

const HIGHLIGHT_SCROLL_TOP_GAP = 8;

function scrollElementToTop(
  container: HTMLElement,
  target: HTMLElement,
  smooth = true,
) {
  const containerRect = container.getBoundingClientRect();
  const targetRect = target.getBoundingClientRect();
  const offset = targetRect.top - containerRect.top + container.scrollTop;
  const scrollTop = offset - HIGHLIGHT_SCROLL_TOP_GAP;
  container.scrollTo({
    top: Math.max(0, scrollTop),
    behavior: smooth ? "smooth" : "auto",
  });
}

function startScrollWatch(
  session: number,
  container: HTMLElement,
  target: HTMLElement,
) {
  stopScrollWatch();

  const correct = () => {
    if (session !== scrollSession || !props.highlightMessageId) return;
    const currentTarget = document.getElementById(
      `message-${props.highlightMessageId}`,
    );
    if (!currentTarget) return;
    scrollElementToTop(container, currentTarget, false);
  };

  const scheduleCorrection = () => {
    if (session !== scrollSession) return;
    if (resizeTimer) clearTimeout(resizeTimer);
    resizeTimer = setTimeout(correct, 50);
  };

  resizeObserver = new ResizeObserver(scheduleCorrection);
  resizeObserver.observe(container);
  for (const message of container.querySelectorAll("article.message")) {
    resizeObserver.observe(message);
    if (message === target) break;
  }

  scrollWatchTimer = setTimeout(() => {
    if (session === scrollSession) stopScrollWatch();
  }, 4000);
}

async function scrollToHighlightedMessage() {
  const session = ++scrollSession;
  stopScrollWatch();

  if (!props.highlightMessageId || props.loading || props.messages.length === 0) {
    return;
  }

  await nextTick();
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
  if (session !== scrollSession) return;

  const container = messagesContainerRef.value;
  const target = document.getElementById(`message-${props.highlightMessageId}`);
  if (!container || !target) return;

  await waitForImages(layoutAffectingImages(container, target));
  if (session !== scrollSession) return;

  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  if (session !== scrollSession) return;

  const currentTarget = document.getElementById(`message-${props.highlightMessageId}`);
  if (!container || !currentTarget) return;

  scrollElementToTop(container, currentTarget, true);
  startScrollWatch(session, container, currentTarget);
}

watch(
  () => [props.highlightMessageId, props.loading, props.messages] as const,
  () => {
    if (props.highlightMessageId) {
      highlightPulseKey.value += 1;
    }
    void scrollToHighlightedMessage();
  },
);

const galleryImages = computed(() =>
  props.messages.flatMap((message) =>
    message.attachments.map((attachment) => ({
      path: attachment.path,
      fileKey: attachment.file_key,
    })),
  ),
);

function formatTime(timestamp: number | null) {
  if (!timestamp) return "";
  return formatDateTime(timestamp, locale.value as AppLocale);
}

function assistantLabel(source?: string | null) {
  switch (source?.toLowerCase()) {
    case "cursor":
      return "Cursor";
    case "codex":
      return "Codex";
    case "claude":
      return "Claude";
    case "gemini":
      return "Gemini";
    case "deepseek":
      return "DeepSeek";
    case "copilot":
      return "Copilot";
    case "grok":
      return "Grok";
    case "chatgpt":
      return "ChatGPT";
    default:
      return t("common.assistant");
  }
}

function roleLabel(role: string) {
  if (role === "user") return t("common.you");
  if (role === "assistant") return assistantLabel(props.dataSource);
  if (role === "system") {
    return props.dataSource?.toLowerCase() === "cursor"
      ? "Cursor"
      : t("common.system");
  }
  return role;
}

function sanitizeContent(content: string) {
  return content
    .split("\n")
    .filter((line) => {
      const trimmed = line.trim();
      return trimmed !== "[image_asset_pointer]" && trimmed !== "[multimodal_text]" && trimmed !== "[user_editable_context]";
    })
    .join("\n")
    .trim();
}

function imageSrc(path: string) {
  if (!imageSrcCache[path]) {
    imageSrcCache[path] = convertFileSrc(path);
  }
  return imageSrcCache[path];
}

async function onImageError(path: string) {
  if (imageSrcCache[path]?.startsWith("data:")) {
    return;
  }
  try {
    imageSrcCache[path] = await readImageDataUrl(path);
  } catch {
    imageSrcCache[path] = "";
  }
}

function openLightbox(path: string) {
  const index = galleryImages.value.findIndex((item) => item.path === path);
  if (index < 0) return;
  lightboxIndex.value = index;
  lightboxVisible.value = true;
}

const renderedMessages = computed(() =>
  props.messages
    .map((message) => ({
      ...message,
      html: renderMarkdown(sanitizeContent(message.content)),
    }))
    .filter((message) => message.html.trim() || message.attachments.length > 0),
);

const eagerImageMessageIds = computed(() => {
  if (!props.highlightMessageId) return null;
  const ids = new Set<string>();
  for (const message of renderedMessages.value) {
    ids.add(message.id);
    if (message.id === props.highlightMessageId) break;
  }
  return ids;
});

function shouldEagerLoadImages(messageId: string) {
  return eagerImageMessageIds.value?.has(messageId) ?? false;
}
</script>

<template>
  <div class="message-view">
    <div v-if="loading" class="loading-wrap">
      <el-skeleton animated :rows="10" />
    </div>
    <el-empty v-else-if="messages.length === 0" :description="t('conversation.selectPrompt')" />
    <template v-else>
      <header class="header">
        <div class="header-main">
          <h2>{{ title }}</h2>
          <div class="header-meta">
            <span class="count">{{ t("common.messages", { count: renderedMessages.length }) }}</span>
            <template v-if="showInfoButton">
              <span class="meta-sep" aria-hidden="true">·</span>
              <button
                type="button"
                class="info-link"
                :aria-label="t('conversation.infoAria')"
                @click="emit('openConversationInfo')"
              >
                {{ t("conversation.info") }}
              </button>
            </template>
            <el-tag
              v-for="tag in conversationTags"
              :key="tag"
              size="small"
              type="info"
              effect="plain"
            >
              {{ tag }}
            </el-tag>
          </div>
        </div>
        <div class="header-actions">
          <el-button
            :type="conversationStarred ? 'warning' : 'default'"
            text
            @click="emit('toggleConversationStar')"
          >
            {{ conversationStarred ? t("conversation.starred") : t("conversation.star") }}
          </el-button>
          <el-button text @click="emit('exportMarkdown')">{{ t("conversation.exportMarkdown") }}</el-button>
        </div>
      </header>
      <div ref="messagesContainerRef" class="messages">
        <article
          v-for="message in renderedMessages"
          :key="message.id"
          :id="`message-${message.id}`"
          class="message"
          tabindex="0"
          :class="[
            message.role,
            {
              starred: message.is_starred,
              highlighted: message.id === highlightMessageId,
            },
          ]"
          :data-pulse="
            message.id === highlightMessageId ? highlightPulseKey : undefined
          "
        >
          <div class="message-head">
            <div class="head-left">
              <span class="role">{{ roleLabel(message.role) }}</span>
              <span
                v-if="message.id === highlightMessageId"
                class="search-match-badge"
              >
                {{ t("search.matched") }}
              </span>
              <el-button
                class="star-btn"
                text
                size="small"
                :type="message.is_starred ? 'warning' : 'default'"
                @click="
                  emit('toggleMessageStar', message.id, !message.is_starred)
                "
              >
                {{ message.is_starred ? "★" : "☆" }}
              </el-button>
            </div>
            <span class="time">{{ formatTime(message.create_time) }}</span>
          </div>
          <div
            v-if="message.html.trim()"
            class="content message-markdown selectable-text"
            v-html="message.html"
          />
          <div v-if="message.attachments.length" class="attachments">
            <figure
              v-for="attachment in message.attachments"
              :key="attachmentCacheKey(attachment)"
              class="attachment"
            >
              <img
                v-if="imageSrcCache[attachment.path] !== ''"
                :src="imageSrc(attachment.path)"
                :alt="attachment.file_key"
                class="thumb"
                :loading="shouldEagerLoadImages(message.id) ? 'eager' : 'lazy'"
                @error="onImageError(attachment.path)"
                @click="openLightbox(attachment.path)"
              />
              <div v-else class="image-missing">
                {{ t("conversation.imageLoadFailed", { key: attachment.file_key }) }}
              </div>
            </figure>
          </div>
        </article>
      </div>
    </template>

    <ImageLightbox
      v-model:visible="lightboxVisible"
      :images="galleryImages"
      :initial-index="lightboxIndex"
      :resolve-src="imageSrc"
      :on-image-error="onImageError"
    />
  </div>
</template>

<style scoped>
.message-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--cl-panel);
  min-height: 0;
}

.loading-wrap {
  padding: 24px;
}

.header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 20px 10px;
  border-bottom: 1px solid var(--cl-border-subtle);
  background: var(--cl-panel-elevated);
  flex-shrink: 0;
}

.header-main h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--cl-text);
}

.header-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.count {
  font-size: 12px;
  color: var(--cl-text-muted);
}

.meta-sep {
  font-size: 12px;
  color: var(--cl-text-faint);
}

.info-link {
  border: none;
  background: transparent;
  padding: 0;
  font-size: 12px;
  color: var(--cl-text-muted);
  cursor: pointer;
}

.info-link:hover {
  color: var(--cl-text);
  text-decoration: underline;
}

.header-actions {
  display: flex;
  align-items: flex-start;
  gap: 4px;
  flex-shrink: 0;
}

.messages {
  flex: 1;
  overflow: auto;
  padding: 8px 20px 24px;
  background: var(--cl-panel-elevated);
}

.message {
  position: relative;
  max-width: var(--cl-content-max-width);
  margin: 0 auto;
  padding: 12px 0;
  border-radius: 0;
  border: none;
  border-bottom: 1px solid var(--cl-border-subtle);
  background: transparent;
  outline: none;
}

.message:focus-visible {
  outline: 1px solid var(--cl-accent);
  outline-offset: 2px;
}

.message.starred {
  border-bottom-color: rgba(201, 162, 39, 0.35);
}

.message.highlighted {
  margin-top: 12px;
  margin-bottom: 12px;
  padding: 14px 16px 14px 16px;
  border-bottom-color: transparent;
  border-left: 4px solid var(--cl-search-spotlight-bar);
  background: var(--cl-search-spotlight-bg);
  border-radius: 10px;
  box-shadow: var(--cl-search-spotlight-shadow);
  scroll-margin-top: 8px;
}

.message.highlighted[data-pulse] {
  animation: search-hit-enter 1.2s cubic-bezier(0.22, 1, 0.36, 1);
}

@keyframes search-hit-enter {
  0% {
    background: color-mix(in srgb, var(--cl-search-spotlight-bar) 28%, var(--cl-panel-elevated));
    box-shadow:
      0 0 0 2px var(--cl-search-spotlight-ring),
      0 10px 28px rgba(91, 107, 130, 0.22);
  }
  100% {
    background: var(--cl-search-spotlight-bg);
    box-shadow: var(--cl-search-spotlight-shadow);
  }
}

.message.user {
  margin-top: 4px;
  margin-bottom: 4px;
  padding: 12px 14px;
  border: 1px solid var(--cl-border-subtle);
  border-radius: 8px;
  border-bottom: 1px solid var(--cl-border-subtle);
  background: var(--cl-message-user-bg);
}

.message.assistant,
.message.system {
  padding: 14px 0;
}

.message.user.highlighted {
  margin-top: 12px;
  margin-bottom: 12px;
  background: var(--cl-search-spotlight-bg);
  border: 1px solid var(--cl-search-spotlight-ring);
  border-left: 4px solid var(--cl-search-spotlight-bar);
  border-radius: 10px;
  padding: 12px 14px;
  box-shadow: var(--cl-search-spotlight-shadow);
}

.message.assistant.highlighted,
.message.system.highlighted {
  padding: 14px 16px;
  border-top: 1px solid var(--cl-search-spotlight-ring);
  border-right: 1px solid var(--cl-search-spotlight-ring);
  border-bottom: 1px solid var(--cl-search-spotlight-ring);
}

.message-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
  font-size: 11px;
  color: var(--cl-text-faint);
}

.message.user .message-head {
  margin-bottom: 6px;
}

.head-left {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.search-match-badge {
  display: inline-flex;
  align-items: center;
  padding: 1px 7px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--cl-search-spotlight-bar);
  background: var(--cl-accent-soft);
  border: 1px solid var(--cl-search-spotlight-ring);
}

.role {
  font-weight: 500;
  color: var(--cl-text-muted);
}

.message.user .role {
  color: var(--cl-text);
  font-weight: 600;
}

.star-btn {
  min-height: auto;
  padding: 0 4px;
}

.content {
  font-size: 14.5px;
  line-height: 1.6;
  color: var(--cl-text);
}

.content :deep(p) {
  margin: 0.45em 0;
}

.content :deep(p:last-child) {
  margin-bottom: 0;
}

.content :deep(h1) {
  font-size: 18px;
  line-height: 1.35;
  font-weight: 700;
  margin: 1.1em 0 0.55em;
}

.content :deep(h2) {
  font-size: 16.5px;
  line-height: 1.4;
  font-weight: 700;
  margin: 1em 0 0.5em;
  border-bottom: none;
}

.content :deep(h3) {
  font-size: 15.5px;
  line-height: 1.4;
  font-weight: 600;
  margin: 0.9em 0 0.45em;
}

.content :deep(h4),
.content :deep(h5),
.content :deep(h6) {
  font-size: 15px;
  line-height: 1.4;
  font-weight: 600;
  margin: 0.85em 0 0.4em;
}

.content :deep(ul),
.content :deep(ol) {
  margin: 0.45em 0 0.65em;
  padding-left: 1.35em;
}

.content :deep(li) {
  margin: 0.25em 0;
}

.content :deep(hr) {
  margin: 14px 0;
  border: 0;
  border-top: 1px solid var(--cl-border-subtle);
}

.content :deep(blockquote) {
  margin: 0.65em 0;
  padding-left: 12px;
  border-left: 3px solid var(--cl-border);
  color: var(--cl-text-muted);
}

.content :deep(pre) {
  overflow: auto;
  padding: 10px 12px;
  border-radius: 8px;
  margin: 0.65em 0;
  font-size: 13px;
  line-height: 1.55;
  background: var(--cl-code-bg);
  color: var(--cl-code-text);
}

.content :deep(code) {
  font-family: Consolas, "Courier New", monospace;
  font-size: 13px;
}

.content :deep(p code) {
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(127, 127, 127, 0.15);
}

.message.user .content {
  line-height: 1.58;
}

.attachments {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 12px;
}

.attachment {
  margin: 0;
}

.attachment img,
.attachment .thumb {
  max-width: min(100%, 480px);
  border-radius: 8px;
  border: 1px solid var(--cl-border);
}

.thumb {
  cursor: zoom-in;
}

.image-missing {
  font-size: 12px;
  color: var(--cl-text-muted);
  padding: 8px 12px;
  border: 1px dashed var(--cl-border);
  border-radius: 8px;
}
</style>
