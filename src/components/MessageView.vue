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
}>();

const emit = defineEmits<{
  toggleConversationStar: [];
  exportMarkdown: [];
  toggleMessageStar: [messageId: string, starred: boolean];
}>();

const { t, locale } = useI18n();

const imageSrcCache = reactive<Record<string, string>>({});
const lightboxVisible = ref(false);
const lightboxIndex = ref(0);
const messagesContainerRef = ref<HTMLElement | null>(null);

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

function scrollElementIntoCenter(
  container: HTMLElement,
  target: HTMLElement,
  smooth = true,
) {
  const containerRect = container.getBoundingClientRect();
  const targetRect = target.getBoundingClientRect();
  const offset = targetRect.top - containerRect.top + container.scrollTop;
  const scrollTop =
    offset - container.clientHeight / 2 + target.clientHeight / 2;
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
    scrollElementIntoCenter(container, currentTarget, false);
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

  scrollElementIntoCenter(container, currentTarget, true);
  startScrollWatch(session, container, currentTarget);
}

watch(
  () => [props.highlightMessageId, props.loading, props.messages] as const,
  () => {
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
          :class="[
            message.role,
            {
              starred: message.is_starred,
              highlighted: message.id === highlightMessageId,
            },
          ]"
        >
          <div class="message-head">
            <div class="head-left">
              <span class="role">{{ roleLabel(message.role) }}</span>
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
            class="content markdown-body"
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
}

.loading-wrap {
  padding: 24px;
}

.header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding: 20px 24px 12px;
  border-bottom: 1px solid var(--cl-border);
}

.header-main h2 {
  margin: 0;
  font-size: 18px;
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

.header-actions {
  display: flex;
  align-items: flex-start;
  gap: 4px;
  flex-shrink: 0;
}

.messages {
  flex: 1;
  overflow: auto;
  padding: 16px 24px 32px;
}

.message {
  max-width: 860px;
  margin: 0 auto 20px;
  padding: 16px 18px;
  border-radius: 12px;
  border: 1px solid var(--cl-border);
  background: var(--cl-message-bg);
}

.message.starred {
  border-color: rgba(230, 162, 60, 0.45);
}

.message.highlighted {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.15);
}

.message.user {
  margin-left: auto;
  background: rgba(64, 158, 255, 0.08);
}

.message-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
  font-size: 12px;
  color: var(--cl-text-muted);
}

.head-left {
  display: flex;
  align-items: center;
  gap: 4px;
}

.role {
  font-weight: 600;
  color: var(--cl-text);
}

.star-btn {
  min-height: auto;
  padding: 0 4px;
}

.content :deep(p) {
  margin: 0 0 0.8em;
}

.content :deep(pre) {
  overflow: auto;
  padding: 12px;
  border-radius: 8px;
  background: var(--cl-code-bg);
  color: var(--cl-code-text);
}

.content :deep(code) {
  font-family: Consolas, "Courier New", monospace;
}

.content :deep(p code) {
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(127, 127, 127, 0.15);
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
