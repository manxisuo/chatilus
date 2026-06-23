<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import ConversationList from "./components/ConversationList.vue";
import ConversationInfo from "./components/ConversationInfo.vue";
import ImageGallery from "./components/ImageGallery.vue";
import MessageView from "./components/MessageView.vue";
import TagDialog from "./components/TagDialog.vue";
import {
  createTag,
  deleteTag,
  exportConversationMarkdown,
  getMessages,
  getStats,
  listConversations,
  listTags,
  searchMessages,
  setConversationStarred,
  setConversationTags,
  setMessageStarred,
  startImport,
} from "./api";
import type {
  ConversationSummary,
  DatabaseStats,
  ImportJobView,
  ImportProgressEvent,
  ImportResult,
  SearchHit,
  TagView,
} from "./types";
import { KNOWN_DATA_SOURCES, sourceLabel, sourceTagType } from "./utils/dataSource";

const conversations = ref<ConversationSummary[]>([]);
const messages = ref<Awaited<ReturnType<typeof getMessages>>>([]);
const searchHits = ref<SearchHit[]>([]);
const tags = ref<TagView[]>([]);
const activeId = ref<string | null>(null);
const listLoading = ref(false);
const messageLoading = ref(false);
const importing = ref(false);
const importDialogVisible = ref(false);
const importJobId = ref<string | null>(null);
const importProgress = ref({
  phase: "pending",
  progress: 0,
  processed: 0,
  total: 0,
});
let importUnlisten: UnlistenFn[] = [];
const exporting = ref(false);
const stats = ref<DatabaseStats | null>(null);
const listQuery = ref("");
const searchQuery = ref("");
const searchMode = ref(false);
const starredOnly = ref(false);
const filterHasImages = ref(false);
const filterHasCode = ref(false);
const filterHasAttachments = ref(false);
const filterTagId = ref<number | null>(null);
const filterSource = ref<string | null>(null);
const tagDialogVisible = ref(false);
const viewMode = ref<"chats" | "images">("chats");

const sourceCount = computed(() => {
  const entries = stats.value?.conversation_counts_by_source ?? [];
  return entries.filter((item) => item.count > 0).length;
});

function formatStatsTime(timestamp: number | null | undefined) {
  if (!timestamp) return "—";
  return new Date(timestamp * 1000).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

const dashboardStats = computed(() => {
  if (!stats.value) return [];
  const s = stats.value;
  return [
    { label: "对话", value: s.conversation_count, icon: "💬" },
    { label: "图片", value: s.image_count, icon: "🖼" },
    { label: "来源", value: sourceCount.value, icon: "📦" },
    { label: "收藏对话", value: s.starred_conversation_count, icon: "★" },
    { label: "标签", value: s.tag_count, icon: "🏷" },
    {
      label: "最近导入",
      value: formatStatsTime(s.last_imported_at),
      icon: "📥",
      isText: true,
    },
  ];
});

const navItems = computed(() => [
  {
    id: "chats" as const,
    icon: "💬",
    label: "对话",
    count: stats.value?.conversation_count ?? null,
  },
  {
    id: "images" as const,
    icon: "🖼",
    label: "图片",
    count: stats.value?.image_count ?? null,
  },
]);

const PAGE_SIZE = 100;
const listOffset = ref(0);
const hasMoreConversations = ref(true);
const loadingMore = ref(false);

const listTotalHint = computed(() => {
  if (
    starredOnly.value ||
    filterHasImages.value ||
    filterHasCode.value ||
    filterHasAttachments.value ||
    filterTagId.value ||
    listQuery.value.trim()
  ) {
    return null;
  }
  if (filterSource.value) {
    const entry = stats.value?.conversation_counts_by_source?.find(
      (item) => item.source === filterSource.value,
    );
    return entry?.count ?? null;
  }
  return stats.value?.conversation_count ?? null;
});

const sourceNavItems = computed(() => {
  const counts = new Map(
    (stats.value?.conversation_counts_by_source ?? []).map((item) => [
      item.source,
      item.count,
    ]),
  );
  const items: Array<{ id: string | null; label: string; count: number }> = [
    {
      id: null,
      label: "全部对话",
      count: stats.value?.conversation_count ?? 0,
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

  for (const entry of stats.value?.conversation_counts_by_source ?? []) {
    if (seen.has(entry.source)) {
      continue;
    }
    items.push({
      id: entry.source,
      label: sourceLabel(entry.source),
      count: entry.count,
    });
  }

  return items;
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
      hasImages: filterHasImages.value,
      hasCode: filterHasCode.value,
      hasAttachments: filterHasAttachments.value,
      tagId: filterTagId.value,
      source: filterSource.value,
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

const importPhaseLabel = computed(() => {
  switch (importProgress.value.phase) {
    case "extracting":
      return "正在解压…";
    case "parsing":
      return "正在解析对话…";
    case "persisting":
      return "正在写入数据库…";
    case "done":
      return "导入完成";
    default:
      return "准备导入…";
  }
});

function cleanupImportListeners() {
  for (const unlisten of importUnlisten) {
    void unlisten();
  }
  importUnlisten = [];
}

onUnmounted(() => {
  cleanupImportListeners();
});

function formatImportResultMessage(result: ImportResult): string {
  const parts: string[] = [];
  if (result.conversations_imported > 0) {
    parts.push(`新增 ${result.conversations_imported} 个对话`);
  }
  if (result.conversations_updated > 0) {
    parts.push(`更新 ${result.conversations_updated} 个对话`);
  }
  if (result.conversations_deduplicated > 0) {
    parts.push(`包内合并 ${result.conversations_deduplicated} 个重复对话`);
  }
  if (parts.length === 0) {
    parts.push("未发现对话");
  }
  parts.push(`写入 ${result.messages_imported} 条消息`);
  parts.push(`索引媒体 ${result.media_files_indexed} 个`);
  return `导入完成：${parts.join("，")}`;
}

async function finishImport(job: ImportJobView) {
  importing.value = false;
  importDialogVisible.value = false;
  cleanupImportListeners();
  importJobId.value = null;

  if (job.status === "done" && job.result) {
    ElMessage.success(formatImportResultMessage(job.result));
    searchMode.value = false;
    searchQuery.value = "";
    searchHits.value = [];
    await refreshStats();
    await refreshTags();
    await loadConversations();
    activeId.value = conversations.value[0]?.id ?? null;
    return;
  }

  ElMessage.error(job.error ?? "导入失败");
}

async function startImportFlow(path: string) {
  cleanupImportListeners();
  importing.value = true;
  importDialogVisible.value = true;
  importProgress.value = {
    phase: "pending",
    progress: 0,
    processed: 0,
    total: 0,
  };

  try {
    const jobId = await startImport(path);
    importJobId.value = jobId;

    importUnlisten.push(
      await listen<ImportProgressEvent>("import-progress", (event) => {
        if (event.payload.job_id !== importJobId.value) {
          return;
        }
        importProgress.value = {
          phase: event.payload.phase,
          progress: Math.round(event.payload.progress * 100),
          processed: event.payload.processed,
          total: event.payload.total,
        };
      }),
    );

    importUnlisten.push(
      await listen<ImportJobView>("import-complete", (event) => {
        if (event.payload.id !== importJobId.value) {
          return;
        }
        void finishImport(event.payload);
      }),
    );
  } catch (error) {
    importing.value = false;
    importDialogVisible.value = false;
    cleanupImportListeners();
    ElMessage.error(String(error));
  }
}

async function handleImportFile() {
  const selected = await open({
    multiple: false,
    title: "选择导入文件",
    filters: [
      {
        name: "支持的导入文件",
        extensions: ["zip", "vscdb"],
      },
    ],
  });

  if (!selected || Array.isArray(selected)) {
    return;
  }

  await startImportFlow(selected);
}

async function handleImportDir() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择导入目录（ChatGPT 导出 / Cursor User 或 globalStorage）",
  });

  if (!selected || Array.isArray(selected)) {
    return;
  }

  await startImportFlow(selected);
}

function handleImportCommand(command: string) {
  if (command === "file") {
    void handleImportFile();
    return;
  }
  void handleImportDir();
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

function formatHitTime(timestamp: number | null) {
  if (!timestamp) return "";
  return new Date(timestamp * 1000).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function searchHitRoleLabel(hit: SearchHit) {
  if (hit.role === "user") return "你";
  if (hit.role === "assistant") return sourceLabel(hit.source ?? "chatgpt");
  if (hit.role === "system") {
    return hit.source?.toLowerCase() === "cursor" ? "Cursor" : "系统";
  }
  return hit.role;
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

watch(
  [listQuery, starredOnly, filterHasImages, filterHasCode, filterHasAttachments, filterTagId, filterSource],
  async () => {
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
        <div class="brand-text">
          <strong class="brand-name">ChatLens</strong>
          <span class="brand-tagline">Browse your AI memory</span>
        </div>
        <nav class="nav-tabs" aria-label="主视图">
          <button
            v-for="item in navItems"
            :key="item.id"
            class="nav-tab"
            :class="{ active: viewMode === item.id }"
            type="button"
            @click="viewMode = item.id"
          >
            <span class="nav-icon" aria-hidden="true">{{ item.icon }}</span>
            <span class="nav-label">{{ item.label }}</span>
            <span v-if="item.count" class="nav-count">{{ item.count.toLocaleString() }}</span>
          </button>
        </nav>
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
        <el-dropdown trigger="click" @command="handleImportCommand">
          <el-button type="primary" :loading="importing">
            导入数据
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="file">导入文件（ZIP / Cursor .vscdb / Gemini Takeout）</el-dropdown-item>
              <el-dropdown-item command="dir">导入目录</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </el-header>

    <div v-if="stats" class="stats-bar" aria-label="库统计">
      <div
        v-for="item in dashboardStats"
        :key="item.label"
        class="stat-item"
      >
        <span class="stat-icon" aria-hidden="true">{{ item.icon }}</span>
        <span class="stat-label">{{ item.label }}</span>
        <span class="stat-value">
          {{ item.isText ? item.value : Number(item.value).toLocaleString() }}
        </span>
      </div>
    </div>

    <el-dialog
      v-model="importDialogVisible"
      title="正在导入"
      width="420px"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
      :show-close="false"
    >
      <p>{{ importPhaseLabel }}</p>
      <el-progress
        :percentage="importProgress.progress"
        :stroke-width="16"
        striped
        striped-flow
      />
      <p v-if="importProgress.total > 0" class="import-progress-detail">
        {{ importProgress.processed }} / {{ importProgress.total }}
      </p>
    </el-dialog>

    <el-container v-if="viewMode === 'chats'" class="body">
      <el-aside width="320px" class="sidebar">
        <div v-if="!searchMode" class="source-nav">
          <div class="section-title">来源</div>
          <button
            v-for="item in sourceNavItems"
            :key="item.id ?? 'all'"
            type="button"
            class="source-nav-item"
            :class="{ active: filterSource === item.id }"
            @click="filterSource = item.id"
          >
            <span>{{ item.label }}</span>
            <span class="source-nav-count">{{ item.count }}</span>
          </button>
        </div>

        <div class="sidebar-tools">
          <div class="section-title">筛选</div>
          <el-input
            v-model="listQuery"
            clearable
            placeholder="筛选对话标题…"
            :disabled="searchMode"
          />
          <div class="filter-group">
            <el-checkbox v-model="starredOnly" :disabled="searchMode">
              收藏
            </el-checkbox>
            <el-checkbox v-model="filterHasImages" :disabled="searchMode">
              有图片
            </el-checkbox>
            <el-checkbox v-model="filterHasCode" :disabled="searchMode">
              有代码
            </el-checkbox>
            <el-checkbox v-model="filterHasAttachments" :disabled="searchMode">
              有附件
            </el-checkbox>
          </div>
        </div>

        <div v-if="!searchMode && tags.length > 0" class="tag-section">
          <div class="section-title">标签</div>
          <el-select
            v-model="filterTagId"
            clearable
            placeholder="按标签筛选"
            size="small"
            class="tag-filter"
          >
            <el-option
              v-for="tag in tags"
              :key="tag.id"
              :label="`${tag.name} (${tag.conversation_count})`"
              :value="tag.id"
            />
          </el-select>
        </div>

        <div v-if="searchMode" class="search-results">
          <div class="section-title">
            搜索结果
            <span v-if="searchHits.length > 0" class="search-count">
              {{ searchHits.length }} 条
            </span>
          </div>
          <el-empty v-if="searchHits.length === 0" description="没有匹配的消息" />
          <button
            v-for="hit in searchHits"
            :key="hit.message_id"
            class="search-hit"
            @click="openSearchHit(hit)"
          >
            <div class="hit-header">
              <el-tag
                size="small"
                :type="sourceTagType(hit.source)"
                effect="plain"
              >
                {{ sourceLabel(hit.source) }}
              </el-tag>
              <div class="hit-title">{{ hit.conversation_title }}</div>
            </div>
            <div class="hit-meta">
              <span class="hit-role">{{ searchHitRoleLabel(hit) }}</span>
              <span v-if="hit.create_time" class="hit-time">
                {{ formatHitTime(hit.create_time) }}
              </span>
            </div>
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
        <div class="main-layout">
          <MessageView
            :messages="messages"
            :title="activeConversation?.title ?? '对话详情'"
            :data-source="activeConversation?.source ?? 'chatgpt'"
            :loading="messageLoading || exporting"
            :conversation-starred="activeConversation?.is_starred ?? false"
            :conversation-tags="activeConversation?.tags ?? []"
            @toggle-conversation-star="toggleConversationStar"
            @export-markdown="handleExportMarkdown"
            @toggle-message-star="toggleMessageStar"
          />
          <ConversationInfo
            v-if="activeConversation"
            :conversation="activeConversation"
            :messages="messages"
          />
        </div>
        <div v-if="activeConversation" class="tag-fab">
          <el-button size="small" @click="tagDialogVisible = true">管理标签</el-button>
        </div>
      </el-main>
    </el-container>

    <el-main v-else class="main gallery-main">
      <ImageGallery
        :key="`${stats?.image_count ?? 0}:${filterSource ?? 'all'}`"
        v-model:filter-source="filterSource"
        :total-count="stats?.image_count ?? null"
        :generated-count="stats?.generated_image_count ?? null"
        :upload-count="stats?.upload_image_count ?? null"
        :image-counts-by-source="stats?.image_counts_by_source ?? []"
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
  display: flex;
  flex-direction: column;
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  border-bottom: 1px solid var(--cl-border);
  background: var(--cl-panel);
}

.stats-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 20px;
  padding: 8px 20px;
  border-bottom: 1px solid var(--cl-border);
  background: var(--cl-bg);
}

.stat-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--cl-text-muted);
}

.stat-icon {
  font-size: 13px;
  line-height: 1;
}

.stat-label {
  font-weight: 500;
}

.stat-value {
  font-variant-numeric: tabular-nums;
  color: var(--cl-text);
  font-weight: 600;
}

.brand {
  display: flex;
  align-items: center;
  gap: 20px;
  min-width: 0;
}

.brand-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  flex-shrink: 0;
}

.brand-tagline {
  font-size: 11px;
  color: var(--cl-text-muted);
  letter-spacing: 0.01em;
}

.brand-name {
  font-size: 17px;
  font-weight: 700;
  color: var(--cl-text);
  letter-spacing: -0.02em;
  flex-shrink: 0;
}

.nav-tabs {
  display: flex;
  gap: 2px;
  padding: 3px;
  border-radius: 10px;
  background: var(--cl-bg);
  border: 1px solid var(--cl-border);
}

.nav-tab {
  border: none;
  background: transparent;
  padding: 6px 14px;
  border-radius: 8px;
  font-size: 13px;
  color: var(--cl-text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  transition: background 0.15s ease, color 0.15s ease;
}

.nav-tab:hover {
  color: var(--cl-text);
}

.nav-tab.active {
  background: var(--cl-panel);
  color: var(--cl-text);
  font-weight: 600;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
}

.nav-icon {
  font-size: 14px;
  line-height: 1;
}

.nav-label {
  line-height: 1.2;
}

.nav-count {
  font-size: 11px;
  font-weight: 500;
  color: var(--cl-text-muted);
  font-variant-numeric: tabular-nums;
}

.nav-tab.active .nav-count {
  color: var(--el-color-primary);
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
  flex: 1;
}

.sidebar {
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--cl-border);
  background: var(--cl-panel);
  min-height: 0;
  overflow: hidden;
}

.source-nav {
  padding: 8px 0 4px;
  border-bottom: 1px solid var(--cl-border);
}

.source-nav-item {
  width: 100%;
  border: none;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 16px;
  font-size: 13px;
  color: var(--cl-text);
  cursor: pointer;
  text-align: left;
}

.source-nav-item:hover {
  background: rgba(64, 158, 255, 0.06);
}

.source-nav-item.active {
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

.sidebar-tools .section-title {
  padding: 0 0 4px;
}

.filter-group {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px 8px;
}

.tag-section {
  padding: 0 12px 12px;
  border-bottom: 1px solid var(--cl-border);
}

.tag-section .section-title {
  padding: 12px 0 8px;
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
  font-weight: 600;
}

.search-results .section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.search-count {
  font-weight: 500;
  font-size: 11px;
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

.hit-header {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.hit-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hit-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  font-size: 11px;
  color: var(--cl-text-muted);
}

.hit-role {
  font-weight: 600;
  color: var(--cl-text);
}

.hit-time {
  font-variant-numeric: tabular-nums;
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
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.main-layout {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.main-layout :deep(.message-view) {
  flex: 1;
  min-width: 0;
}

.gallery-main {
  padding: 0;
  overflow: hidden;
  flex: 1;
  min-height: 0;
}

.tag-fab {
  position: absolute;
  right: 20px;
  bottom: 20px;
}

.import-progress-detail {
  margin-top: 8px;
  font-size: 12px;
  color: var(--cl-text-muted);
}
</style>
