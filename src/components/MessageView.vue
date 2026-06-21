<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readImageDataUrl } from "../api";
import { renderMarkdown } from "../utils/markdown";
import ImageLightbox from "./ImageLightbox.vue";
import type { MessageView as MessageItem } from "../types";

const props = defineProps<{
  messages: MessageItem[];
  title: string;
  loading: boolean;
  conversationStarred: boolean;
  conversationTags: string[];
}>();

const emit = defineEmits<{
  toggleConversationStar: [];
  exportMarkdown: [];
  toggleMessageStar: [messageId: string, starred: boolean];
}>();

const imageSrcCache = reactive<Record<string, string>>({});
const lightboxVisible = ref(false);
const lightboxIndex = ref(0);

const galleryImages = computed(() =>
  props.messages.flatMap((message) =>
    message.attachments.map((attachment) => ({
      path: attachment.path,
      fileKey: attachment.file_key,
    })),
  ),
);

watch(
  () => props.messages,
  () => {
    lightboxVisible.value = false;
  },
);

function formatTime(timestamp: number | null) {
  if (!timestamp) return "";
  return new Date(timestamp * 1000).toLocaleString("zh-CN");
}

function roleLabel(role: string) {
  if (role === "user") return "你";
  if (role === "assistant") return "ChatGPT";
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
</script>

<template>
  <div class="message-view">
    <div v-if="loading" class="loading-wrap">
      <el-skeleton animated :rows="10" />
    </div>
    <el-empty v-else-if="messages.length === 0" description="选择左侧对话以查看消息" />
    <template v-else>
      <header class="header">
        <div class="header-main">
          <h2>{{ title }}</h2>
          <div class="header-meta">
            <span class="count">{{ renderedMessages.length }} 条消息</span>
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
            {{ conversationStarred ? "★ 已收藏" : "☆ 收藏对话" }}
          </el-button>
          <el-button text @click="emit('exportMarkdown')">导出 Markdown</el-button>
        </div>
      </header>
      <div class="messages">
        <article
          v-for="message in renderedMessages"
          :key="message.id"
          class="message"
          :class="[message.role, { starred: message.is_starred }]"
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
              :key="attachment.path"
              class="attachment"
            >
              <img
                v-if="imageSrcCache[attachment.path] !== ''"
                :src="imageSrc(attachment.path)"
                :alt="attachment.file_key"
                class="thumb"
                loading="lazy"
                @error="onImageError(attachment.path)"
                @click="openLightbox(attachment.path)"
              />
              <div v-else class="image-missing">图片无法加载：{{ attachment.file_key }}</div>
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
  background: #1e1e1e;
  color: #dcdcdc;
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
