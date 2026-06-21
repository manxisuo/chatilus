<script setup lang="ts">
import { computed } from "vue";
import type { ConversationSummary } from "../types";

const props = defineProps<{
  conversations: ConversationSummary[];
  activeId: string | null;
  loading: boolean;
  loadingMore: boolean;
  hasMore: boolean;
  loadedCount: number;
  totalCount: number | null;
}>();

const emit = defineEmits<{
  select: [id: string];
  toggleStar: [id: string, starred: boolean];
  loadMore: [];
}>();

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

const items = computed(() => props.conversations);

function onToggleStar(event: Event, id: string, starred: boolean) {
  event.stopPropagation();
  emit("toggleStar", id, !starred);
}

function onScroll(event: Event) {
  if (!props.hasMore || props.loading || props.loadingMore) {
    return;
  }

  const el = event.target as HTMLElement;
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 120) {
    emit("loadMore");
  }
}

const footerText = computed(() => {
  if (props.loadingMore) {
    return "加载中…";
  }
  if (props.hasMore) {
    if (props.totalCount != null) {
      return `已加载 ${props.loadedCount} / ${props.totalCount}，继续下拉`;
    }
    return `已加载 ${props.loadedCount} 条，继续下拉`;
  }
  if (props.totalCount != null) {
    return `共 ${props.totalCount} 条对话`;
  }
  return `共 ${props.loadedCount} 条对话`;
});
</script>

<template>
  <div class="conversation-list">
    <el-skeleton v-if="loading" animated :rows="8" />
    <el-empty v-else-if="items.length === 0" description="暂无对话，请先导入 ChatGPT 数据" />
    <div v-else class="list-scroll" @scroll.passive="onScroll">
      <button
        v-for="item in items"
        :key="item.id"
        class="conversation-item"
        :class="{ active: item.id === activeId, starred: item.is_starred }"
        @click="emit('select', item.id)"
      >
        <div class="row-top">
          <div class="title">{{ item.title }}</div>
          <span
            class="star"
            :class="{ active: item.is_starred }"
            @click="onToggleStar($event, item.id, item.is_starred)"
          >
            {{ item.is_starred ? "★" : "☆" }}
          </span>
        </div>
        <div v-if="item.tags.length" class="tags">
          <span v-for="tag in item.tags" :key="tag" class="tag">{{ tag }}</span>
        </div>
        <div class="meta">
          <span>{{ formatTime(item.update_time ?? item.create_time) }}</span>
          <span>{{ item.message_count }} 条消息</span>
        </div>
      </button>
      <div class="list-footer">{{ footerText }}</div>
    </div>
  </div>
</template>

<style scoped>
.conversation-list {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.list-scroll {
  overflow: auto;
  flex: 1;
  min-height: 0;
}

.conversation-item {
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 12px 16px;
  cursor: pointer;
  border-bottom: 1px solid var(--cl-border);
}

.conversation-item:hover {
  background: rgba(64, 158, 255, 0.08);
}

.conversation-item.active {
  background: rgba(64, 158, 255, 0.15);
  border-left: 3px solid var(--el-color-primary);
  padding-left: 13px;
}

.conversation-item.starred {
  background: rgba(230, 162, 60, 0.06);
}

.row-top {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.title {
  flex: 1;
  font-size: 14px;
  font-weight: 500;
  color: var(--cl-text);
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.star {
  flex-shrink: 0;
  font-size: 14px;
  color: var(--cl-text-muted);
  padding: 0 2px;
}

.star.active {
  color: #e6a23c;
}

.tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 6px;
}

.tag {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 999px;
  background: rgba(64, 158, 255, 0.12);
  color: var(--el-color-primary);
}

.meta {
  margin-top: 6px;
  display: flex;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
  color: var(--cl-text-muted);
}

.list-footer {
  padding: 12px 16px 16px;
  text-align: center;
  font-size: 12px;
  color: var(--cl-text-muted);
}
</style>
