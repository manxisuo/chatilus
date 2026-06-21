<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import ConversationList from "./components/ConversationList.vue";
import ImageGallery from "./components/ImageGallery.vue";
import MessageView from "./components/MessageView.vue";
import TagDialog from "./components/TagDialog.vue";
import {
  createTag,
  deleteTag,
  exportConversationMarkdown,
  getMessages,
  getStats,
  importExportDir,
  listConversations,
  listTags,
  searchMessages,
  setConversationStarred,
  setConversationTags,
  setMessageStarred,
} from "./api";
import type {
  ConversationSummary,
  DatabaseStats,
  SearchHit,
  TagView,
} from "./types";

const conversations = ref<ConversationSummary[]>([]);
const messages = ref<Awaited<ReturnType<typeof getMessages>>>([]);
const searchHits = ref<SearchHit[]>([]);
const tags = ref<TagView[]>([]);
const activeId = ref<string | null>(null);
const listLoading = ref(false);
const messageLoading = ref(false);
const importing = ref(false);
const exporting = ref(false);
const stats = ref<DatabaseStats | null>(null);
const listQuery = ref("");
const searchQuery = ref("");
const searchMode = ref(false);
const starredOnly = ref(false);
const filterTagId = ref<number | null>(null);
const tagDialogVisible = ref(false);
const viewMode = ref<"chats" | "images">("chats");

const PAGE_SIZE = 100;
const listOffset = ref(0);
const hasMoreConversations = ref(true);
const loadingMore = ref(false);

const listTotalHint = computed(() => {
  if (starredOnly.value || filterTagId.value || listQuery.value.trim()) {
    return null;
  }
  return stats.value?.conversation_count ?? null;
});

const activeConversation = computed(() =>
  conversations.value.find((item) => item.id === activeId.value) ?? null,
);

const activeTagIds = computed(() => {
  if (!activeConversation.value) return [];
  return tags.value
    .filter((tag) => activeConversation.value!.tags.includes(tag.name))
    .map((tag) => tag.id);
});

async function refreshStats() {
  stats.value = await getStats();
}

async function refreshTags() {
  tags.value = await listTags();
}

async function loadConversations(reset = true) {
  if (reset) {
    if (listLoading.value) return;
    listLoading.value = true;
    listOffset.value = 0;
    hasMoreConversations.value = true;
  } else {
    if (listLoading.value || loadingMore.value || !hasMoreConversations.value) {
      return;
    }
    loadingMore.value = true;
  }

  try {
    const batch = await listConversations({
      query: listQuery.value.trim() || undefined,
      starredOnly: starredOnly.value,
      tagId: filterTagId.value,
      limit: PAGE_SIZE,
      offset: listOffset.value,
    });

    if (reset) {
      conversations.value = batch;
    } else {
      const existing = new Set(conversations.value.map((item) => item.id));
      conversations.value = [
        ...conversations.value,
        ...batch.filter((item) => !existing.has(item.id)),
      ];
    }

    listOffset.value = conversations.value.length;
    hasMoreConversations.value = batch.length === PAGE_SIZE;

    if (
      reset &&
      activeId.value &&
      !conversations.value.some((item) => item.id === activeId.value)
    ) {
      activeId.value = conversations.value[0]?.id ?? null;
    }
  } finally {
    listLoading.value = false;
    loadingMore.value = false;
  }
}

async function loadMoreConversations() {
  await loadConversations(false);
}

async function loadMessages(conversationId: string) {
  messageLoading.value = true;
  try {
    messages.value = await getMessages(conversationId);
  } finally {
    messageLoading.value = false;
  }
}

async function handleImport() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择 ChatGPT 导出目录",
  });

  if (!selected || Array.isArray(selected)) {
    return;
  }

  importing.value = true;
  try {
    const result = await importExportDir(selected);
    ElMessage.success(
      `导入完成：${result.conversations_imported} 个对话，${result.messages_imported} 条消息；索引媒体 ${result.media_files_indexed} 个`,
    );
    searchMode.value = false;
    searchQuery.value = "";
    searchHits.value = [];
    await refreshStats();
    await refreshTags();
    await loadConversations();
    activeId.value = conversations.value[0]?.id ?? null;
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    importing.value = false;
  }
}

async function handleSearch() {
  const query = searchQuery.value.trim();
  if (!query) {
    searchMode.value = false;
    searchHits.value = [];
    return;
  }

  searchMode.value = true;
  searchHits.value = await searchMessages(query);
}

function selectConversation(id: string) {
  searchMode.value = false;
  viewMode.value = "chats";
  activeId.value = id;
}

function openConversationFromGallery(conversationId: string) {
  viewMode.value = "chats";
  activeId.value = conversationId;
}

function openSearchHit(hit: SearchHit) {
  searchMode.value = false;
  activeId.value = hit.conversation_id;
}

async function toggleConversationStar() {
  if (!activeConversation.value) return;
  const next = !activeConversation.value.is_starred;
  await setConversationStarred(activeConversation.value.id, next);
  activeConversation.value.is_starred = next;
  await refreshStats();
  await loadConversations();
}

async function toggleConversationStarFromList(id: string, starred: boolean) {
  await setConversationStarred(id, starred);
  const item = conversations.value.find((conv) => conv.id === id);
  if (item) item.is_starred = starred;
  await refreshStats();
}

async function toggleMessageStar(messageId: string, starred: boolean) {
  await setMessageStarred(messageId, starred);
  const item = messages.value.find((message) => message.id === messageId);
  if (item) item.is_starred = starred;
  await refreshStats();
}

async function handleExportMarkdown() {
  if (!activeConversation.value) return;

  const defaultName = `${activeConversation.value.title.replace(/[\\/:*?"<>|]/g, "_")}.md`;
  const outputPath = await save({
    defaultPath: defaultName,
    filters: [{ name: "Markdown", extensions: ["md"] }],
    title: "导出 Markdown",
  });

  if (!outputPath) return;

  exporting.value = true;
  try {
    const result = await exportConversationMarkdown(
      activeConversation.value.id,
      outputPath,
    );
    ElMessage.success(`已导出 ${result.message_count} 条消息`);
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    exporting.value = false;
  }
}

async function handleCreateTag(name: string) {
  try {
    await createTag(name);
    await refreshTags();
    ElMessage.success(`标签「${name}」已创建`);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function handleDeleteTag(tagId: number) {
  try {
    await deleteTag(tagId);
    if (filterTagId.value === tagId) {
      filterTagId.value = null;
    }
    await refreshTags();
    await loadConversations();
    ElMessage.success("标签已删除");
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function handleSaveTags(tagIds: number[]) {
  if (!activeConversation.value) return;
  try {
    const tagNames = await setConversationTags(activeConversation.value.id, tagIds);
    activeConversation.value.tags = tagNames;
    await refreshTags();
    await loadConversations();
    ElMessage.success("标签已更新");
  } catch (error) {
    ElMessage.error(String(error));
  }
}

watch(activeId, async (id) => {
  if (id) {
    await loadMessages(id);
  } else {
    messages.value = [];
  }
});

watch([listQuery, starredOnly, filterTagId], async () => {
  if (!searchMode.value) {
    await loadConversations();
  }
});

onMounted(async () => {
  await refreshStats();
  await refreshTags();
  await loadConversations();
  activeId.value = conversations.value[0]?.id ?? null;
});
</script>

<template>
  <el-container class="app-shell">
    <el-header class="topbar" height="56px">
      <div class="brand">
        <strong>ChatLens</strong>
        <nav class="nav-tabs">
          <button
            class="nav-tab"
            :class="{ active: viewMode === 'chats' }"
            type="button"
            @click="viewMode = 'chats'"
          >
            对话
          </button>
          <button
            class="nav-tab"
            :class="{ active: viewMode === 'images' }"
            type="button"
            @click="viewMode = 'images'"
          >
            图片
            <span v-if="stats?.image_count" class="nav-badge">
              {{ stats.image_count }}
            </span>
          </button>
        </nav>
        <span v-if="stats" class="stats">
          {{ stats.conversation_count }} 对话 · {{ stats.message_count }} 消息
          <template v-if="stats.image_count"> · {{ stats.image_count }} 图片</template>
          <template v-if="stats.starred_conversation_count">
            · ★ {{ stats.starred_conversation_count }}
          </template>
        </span>
      </div>
      <div class="actions">
        <el-input
          v-model="searchQuery"
          clearable
          placeholder="全文搜索消息…"
          class="search-input"
          @keyup.enter="handleSearch"
          @clear="
            searchMode = false;
            searchHits = [];
          "
        />
        <el-button @click="handleSearch">搜索</el-button>
        <el-button type="primary" :loading="importing" @click="handleImport">
          导入导出目录
        </el-button>
      </div>
    </el-header>

    <el-container v-if="viewMode === 'chats'" class="body">
      <el-aside width="320px" class="sidebar">
        <div class="sidebar-tools">
          <el-input
            v-model="listQuery"
            clearable
            placeholder="筛选对话标题…"
            :disabled="searchMode"
          />
          <div class="filters">
            <el-checkbox v-model="starredOnly" :disabled="searchMode">
              只看收藏
            </el-checkbox>
            <el-select
              v-model="filterTagId"
              clearable
              placeholder="按标签筛选"
              :disabled="searchMode"
              size="small"
              class="tag-filter"
            >
              <el-option
                v-for="tag in tags"
                :key="tag.id"
                :label="tag.name"
                :value="tag.id"
              />
            </el-select>
          </div>
        </div>

        <div v-if="searchMode" class="search-results">
          <div class="section-title">搜索结果</div>
          <el-empty v-if="searchHits.length === 0" description="没有匹配的消息" />
          <button
            v-for="hit in searchHits"
            :key="hit.message_id"
            class="search-hit"
            @click="openSearchHit(hit)"
          >
            <div class="hit-title">{{ hit.conversation_title }}</div>
            <div class="hit-snippet" v-html="hit.snippet" />
          </button>
        </div>

        <ConversationList
          v-else
          :conversations="conversations"
          :active-id="activeId"
          :loading="listLoading"
          :loading-more="loadingMore"
          :has-more="hasMoreConversations"
          :loaded-count="conversations.length"
          :total-count="listTotalHint"
          @select="selectConversation"
          @toggle-star="toggleConversationStarFromList"
          @load-more="loadMoreConversations"
        />
      </el-aside>

      <el-main class="main">
        <MessageView
          :messages="messages"
          :title="activeConversation?.title ?? '对话详情'"
          :loading="messageLoading || exporting"
          :conversation-starred="activeConversation?.is_starred ?? false"
          :conversation-tags="activeConversation?.tags ?? []"
          @toggle-conversation-star="toggleConversationStar"
          @export-markdown="handleExportMarkdown"
          @toggle-message-star="toggleMessageStar"
        />
        <div v-if="activeConversation" class="tag-fab">
          <el-button size="small" @click="tagDialogVisible = true">管理标签</el-button>
        </div>
      </el-main>
    </el-container>

    <el-main v-else class="main gallery-main">
      <ImageGallery
        :key="stats?.image_count ?? 0"
        :total-count="stats?.image_count ?? null"
        :generated-count="stats?.generated_image_count ?? null"
        :upload-count="stats?.upload_image_count ?? null"
        @open-conversation="openConversationFromGallery"
      />
    </el-main>

    <TagDialog
      v-model:visible="tagDialogVisible"
      :tags="tags"
      :selected-tag-ids="activeTagIds"
      @save="handleSaveTags"
      @create-tag="handleCreateTag"
      @delete-tag="handleDeleteTag"
    />
  </el-container>
</template>

<style scoped>
.app-shell {
  height: 100vh;
  background: var(--cl-bg);
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  border-bottom: 1px solid var(--cl-border);
  background: var(--cl-panel);
}

.brand {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.nav-tabs {
  display: flex;
  gap: 4px;
}

.nav-tab {
  border: none;
  background: transparent;
  padding: 6px 12px;
  border-radius: 8px;
  font-size: 14px;
  color: var(--cl-text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.nav-tab:hover {
  background: rgba(64, 158, 255, 0.08);
  color: var(--cl-text);
}

.nav-tab.active {
  background: rgba(64, 158, 255, 0.15);
  color: var(--el-color-primary);
  font-weight: 600;
}

.nav-badge {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 999px;
  background: rgba(64, 158, 255, 0.15);
}

.brand strong {
  font-size: 18px;
  color: var(--cl-text);
}

.stats {
  font-size: 12px;
  color: var(--cl-text-muted);
}

.actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.search-input {
  width: 280px;
}

.body {
  min-height: 0;
}

.sidebar {
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--cl-border);
  background: var(--cl-panel);
  min-height: 0;
}

.sidebar > .conversation-list,
.sidebar > .search-results {
  flex: 1;
  min-height: 0;
}

.sidebar-tools {
  padding: 12px;
  border-bottom: 1px solid var(--cl-border);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.filters {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.tag-filter {
  width: 100%;
}

.search-results {
  overflow: auto;
  flex: 1;
}

.section-title {
  padding: 12px 16px 8px;
  font-size: 12px;
  color: var(--cl-text-muted);
}

.search-hit {
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 12px 16px;
  cursor: pointer;
  border-bottom: 1px solid var(--cl-border);
}

.search-hit:hover {
  background: rgba(64, 158, 255, 0.08);
}

.hit-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text);
}

.hit-snippet {
  margin-top: 6px;
  font-size: 12px;
  color: var(--cl-text-muted);
  line-height: 1.5;
}

.main {
  position: relative;
  padding: 0;
  overflow: hidden;
}

.gallery-main {
  padding: 0;
  overflow: hidden;
  height: calc(100vh - 56px);
}

.tag-fab {
  position: absolute;
  right: 20px;
  bottom: 20px;
}
</style>
