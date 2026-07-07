<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ElMessage, type InputInstance } from "element-plus";
import en from "element-plus/es/locale/lang/en";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import ConversationList from "./components/ConversationList.vue";
import ConversationInfo from "./components/ConversationInfo.vue";
import ImageGallery from "./components/ImageGallery.vue";
import SourceNav from "./components/SourceNav.vue";
import WindowControls from "./components/WindowControls.vue";
import MessageView from "./components/MessageView.vue";
import TimelineView from "./components/TimelineView.vue";
import TagDialog from "./components/TagDialog.vue";
import ImportReportDialog from "./components/ImportReportDialog.vue";
import PrivacyDataDialog from "./components/PrivacyDataDialog.vue";
import {
  createTag,
  deleteTag,
  exportConversationMarkdown,
  getMessages,
  getConversation,
  getStats,
  listConversations,
  listStarredMessages,
  listTags,
  searchMessages,
  setConversationStarred,
  listImportGuides,
  setConversationTags,
  setMessageStarred,
  startImport,
} from "./api";
import type {
  ConversationSummary,
  DatabaseStats,
  ImportGuide,
  ImportMethodGuide,
  ImportJobView,
  ImportProgressEvent,
  SearchHit,
  TagView,
} from "./types";
import { KNOWN_DATA_SOURCES, sourceLabel, sourceTagType } from "./utils/dataSource";
import {
  type AppearanceMode,
  getStoredAppearance,
  setAppearance,
} from "./utils/appearance";
import { installDesktopBehaviors } from "./composables/useKeyboardShortcuts";
import { useLayoutBreakpoints } from "./composables/useLayoutBreakpoints";
import { setAppLocale } from "./i18n";
import { LAYOUT_WIDTHS } from "./utils/layout";
import {
  type AppLocale,
  formatDateTime,
} from "./utils/locale";

const { t, locale } = useI18n();

const elementLocale = computed(() => (locale.value === "zh-CN" ? zhCn : en));

const conversations = ref<ConversationSummary[]>([]);
const messages = ref<Awaited<ReturnType<typeof getMessages>>>([]);
const searchHits = ref<SearchHit[]>([]);
const starredMessageHits = ref<SearchHit[]>([]);
const starredMessagesLoading = ref(false);
const tags = ref<TagView[]>([]);
const activeId = ref<string | null>(null);
const fetchedConversation = ref<ConversationSummary | null>(null);
const listLoading = ref(false);
const messageLoading = ref(false);
const importing = ref(false);
const importDialogVisible = ref(false);
const importWizardVisible = ref(false);
const importWizardStep = ref<"source" | "method">("source");
const importGuides = ref<ImportGuide[]>([]);
const selectedImportGuide = ref<ImportGuide | null>(null);
const importJobId = ref<string | null>(null);
const importProgress = ref({
  phase: "pending",
  progress: 0,
  processed: 0,
  total: 0,
});
const importElapsedMs = ref(0);
let importTimer: ReturnType<typeof setInterval> | null = null;
let importUnlisten: UnlistenFn[] = [];
const exporting = ref(false);
const stats = ref<DatabaseStats | null>(null);
const listQuery = ref("");
const searchQuery = ref("");
const searchMode = ref(false);
const activeSearchHitId = ref<string | null>(null);
const starredOnly = ref(false);
const starredMessagesMode = ref(false);
const filterHasImages = ref(false);
const filterHasCode = ref(false);
const filterHasAttachments = ref(false);
const filterTagId = ref<number | null>(null);
const filterSource = ref<string | null>(null);
const tagDialogVisible = ref(false);
const viewMode = ref<"chats" | "timeline" | "images">("chats");
const galleryMonth = ref<string | null>(null);
const galleryConversationId = ref<string | null>(null);
const appearanceMode = ref<AppearanceMode>(getStoredAppearance());
const { showRightPanel, compactLeftPanel } = useLayoutBreakpoints();
const conversationInfoDrawerVisible = ref(false);
const insightPanelWidth = LAYOUT_WIDTHS.insight;
const importReportVisible = ref(false);
const lastImportJob = ref<ImportJobView | null>(null);
const privacyDialogVisible = ref(false);
const searchInputRef = ref<InputInstance>();

const FEEDBACK_URL = "https://github.com/manxisuo/ChatLens/issues";

const appearanceOptions = computed(() => [
  { value: "light" as const, label: t("appearance.light") },
  { value: "dark" as const, label: t("appearance.dark") },
  { value: "system" as const, label: t("appearance.system") },
]);

const languageOptions: Array<{ value: AppLocale; label: string }> = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en", label: "English" },
];

const librarySummaryLine = computed(() => {
  if (!stats.value) return "";
  return t("library.summary", {
    conversations: stats.value.conversation_count.toLocaleString(),
    images: stats.value.image_count.toLocaleString(),
    sources: sourceCount.value,
  });
});

function handleMoreCommand(command: string) {
  if (command === "privacy") {
    privacyDialogVisible.value = true;
    return;
  }
  if (command === "feedback") {
    void openUrl(FEEDBACK_URL);
    return;
  }
  if (command.startsWith("lang:")) {
    handleLanguageCommand(command.slice(5) as AppLocale);
    return;
  }
  if (command.startsWith("appearance:")) {
    handleAppearanceCommand(command.slice(11) as AppearanceMode);
  }
}

function handleAppearanceCommand(mode: AppearanceMode) {
  appearanceMode.value = mode;
  setAppearance(mode);
}

function handleLanguageCommand(next: AppLocale) {
  setAppLocale(next);
}

const sourceCount = computed(() => {
  const entries = stats.value?.conversation_counts_by_source ?? [];
  return entries.filter((item) => item.count > 0).length;
});

const navItems = computed(() => [
  {
    id: "chats" as const,
    icon: "💬",
    label: t("nav.chats"),
    count: stats.value?.conversation_count ?? null,
  },
  {
    id: "timeline" as const,
    icon: "🗓",
    label: t("nav.timeline"),
    count: stats.value?.conversation_count ?? null,
  },
  {
    id: "images" as const,
    icon: "🖼",
    label: t("nav.images"),
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
    starredMessagesMode.value ||
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
      label: t("filter.allConversations"),
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

const visibleSourceNavItems = computed(() =>
  sourceNavItems.value.filter(
    (item) => item.id === null || item.count > 0 || item.id === filterSource.value,
  ),
);

const activeConversation = computed(() =>
  conversations.value.find((item) => item.id === activeId.value) ?? null,
);

const displayConversation = computed(
  () => activeConversation.value ?? fetchedConversation.value,
);

const displayConversationTitle = computed(
  () => displayConversation.value?.title ?? t("conversation.details"),
);

const displayConversationSource = computed(
  () => displayConversation.value?.source ?? "chatgpt",
);

const activeTagIds = computed(() => {
  if (!displayConversation.value) return [];
  return tags.value
    .filter((tag) => displayConversation.value!.tags.includes(tag.name))
    .map((tag) => tag.id);
});

async function refreshStats() {
  stats.value = await getStats();
}

async function refreshTags() {
  tags.value = await listTags();
}

async function loadStarredMessages() {
  starredMessagesLoading.value = true;
  try {
    starredMessageHits.value = await listStarredMessages({
      source: filterSource.value,
      limit: 500,
    });
  } finally {
    starredMessagesLoading.value = false;
  }
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

async function ensureConversationSummary(conversationId: string) {
  if (conversations.value.some((item) => item.id === conversationId)) {
    fetchedConversation.value = null;
    return;
  }
  fetchedConversation.value = await getConversation(conversationId);
}

const importPhaseLabel = computed(() => {
  const phase = importProgress.value.phase;
  if (phase === "extracting" || phase === "parsing" || phase === "persisting" || phase === "done") {
    return t(`import.phase.${phase}`);
  }
  return t("import.phase.pending");
});

function formatImportElapsed(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  const time =
    minutes > 0
      ? t("import.elapsedMinutes", {
          m: minutes,
          s: String(seconds).padStart(2, "0"),
        })
      : t("import.elapsedSeconds", { n: totalSeconds });
  return t("import.elapsed", { time });
}

function startImportTimer() {
  stopImportTimer();
  importElapsedMs.value = 0;
  const startedAt = Date.now();
  importTimer = setInterval(() => {
    importElapsedMs.value = Date.now() - startedAt;
  }, 200);
}

function stopImportTimer() {
  if (importTimer !== null) {
    clearInterval(importTimer);
    importTimer = null;
  }
}

function cleanupImportListeners() {
  stopImportTimer();
  for (const unlisten of importUnlisten) {
    void unlisten();
  }
  importUnlisten = [];
}

let removeKeyboardShortcuts: (() => void) | undefined;

onUnmounted(() => {
  cleanupImportListeners();
  removeKeyboardShortcuts?.();
});

async function finishImport(job: ImportJobView) {
  importing.value = false;
  importDialogVisible.value = false;
  cleanupImportListeners();
  importJobId.value = null;
  lastImportJob.value = job;
  importReportVisible.value = true;

  if (job.status === "done" && job.result) {
    searchMode.value = false;
    searchQuery.value = "";
    searchHits.value = [];
    activeSearchHitId.value = null;
    await refreshStats();
    await refreshTags();
    await loadConversations();
    activeId.value = conversations.value[0]?.id ?? null;
  }
}

function handleImportReportOpenTimeline() {
  viewMode.value = "timeline";
}

function handleImportReportStartSearch() {
  viewMode.value = "chats";
  void nextTick(() => {
    searchInputRef.value?.focus();
  });
}

async function startImportFlow(path: string, importerId?: string) {
  cleanupImportListeners();
  importing.value = true;
  importDialogVisible.value = true;
  startImportTimer();
  importProgress.value = {
    phase: "pending",
    progress: 0,
    processed: 0,
    total: 0,
  };

  try {
    const jobId = await startImport(path, importerId);
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

async function openImportWizard() {
  importWizardStep.value = "source";
  selectedImportGuide.value = null;
  importWizardVisible.value = true;

  try {
    importGuides.value = await listImportGuides();
  } catch (error) {
    importWizardVisible.value = false;
    ElMessage.error(String(error));
  }
}

function importSupportStatusLabel(status: ImportGuide["support_status"]) {
  if (status === "experimental") return t("import.experimental");
  if (status === "beta") return t("import.beta");
  return t("import.stable");
}

function importSupportStatusType(
  status: ImportGuide["support_status"],
): "success" | "warning" | "info" {
  if (status === "experimental") return "warning";
  if (status === "beta") return "info";
  return "success";
}

async function importFromDetectedPath(path: string) {
  const guide = selectedImportGuide.value;
  if (!guide) {
    return;
  }
  importWizardVisible.value = false;
  await startImportFlow(path, guide.importer_id);
}

function selectImportSource(guide: ImportGuide) {
  selectedImportGuide.value = guide;
  importWizardStep.value = "method";
}

function backToImportSources() {
  importWizardStep.value = "source";
  selectedImportGuide.value = null;
}

async function pickImportPath(method: ImportMethodGuide) {
  const guide = selectedImportGuide.value;
  if (!guide) {
    return;
  }

  const isDirectory = method.kind === "directory";
  const extensions = (method.extensions ?? []).filter(Boolean);

  const selected = await open({
    multiple: false,
    directory: isDirectory,
    title: method.dialog_title,
    filters:
      !isDirectory && extensions.length > 0
        ? [{ name: method.label, extensions }]
        : undefined,
  });

  if (!selected || Array.isArray(selected)) {
    return;
  }

  importWizardVisible.value = false;
  await startImportFlow(selected, guide.importer_id);
}

const importWizardTitle = computed(() => {
  if (importWizardStep.value === "source") {
    return t("import.selectSource");
  }
  return t("import.importFrom", {
    name: selectedImportGuide.value?.display_name ?? "",
  });
});

function clearSearch() {
  searchMode.value = false;
  searchHits.value = [];
  activeSearchHitId.value = null;
}

function toggleStarredOnlyFilter() {
  const next = !starredOnly.value;
  starredOnly.value = next;
  if (next) {
    starredMessagesMode.value = false;
  }
}

async function toggleStarredMessagesFilter() {
  const next = !starredMessagesMode.value;
  starredMessagesMode.value = next;
  if (next) {
    starredOnly.value = false;
    searchMode.value = false;
    searchHits.value = [];
    activeSearchHitId.value = null;
    await loadStarredMessages();
  }
}

async function handleSearch() {
  const query = searchQuery.value.trim();
  if (!query) {
    searchMode.value = false;
    searchHits.value = [];
    activeSearchHitId.value = null;
    return;
  }

  searchMode.value = true;
  starredMessagesMode.value = false;
  searchHits.value = await searchMessages(query);
  activeSearchHitId.value = null;
}

function selectConversation(id: string) {
  searchMode.value = false;
  starredMessagesMode.value = false;
  activeSearchHitId.value = null;
  viewMode.value = "chats";
  activeId.value = id;
}

function handleNavClick(id: (typeof navItems.value)[number]["id"]) {
  if (id === "images") {
    galleryMonth.value = null;
    galleryConversationId.value = null;
  }
  viewMode.value = id;
}

function clearGalleryScope() {
  galleryMonth.value = null;
  galleryConversationId.value = null;
}

function openConversationFromGallery(conversationId: string) {
  clearGalleryScope();
  viewMode.value = "chats";
  activeId.value = conversationId;
}

function openConversationFromTimeline(conversationId: string, messageId?: string) {
  searchMode.value = false;
  starredMessagesMode.value = false;
  activeSearchHitId.value = messageId ?? null;
  viewMode.value = "chats";
  activeId.value = conversationId;
}

function openGalleryFromTimeline(options: { month?: string; conversationId?: string }) {
  galleryMonth.value = options.month ?? null;
  galleryConversationId.value = options.conversationId ?? null;
  viewMode.value = "images";
}

function openSearchHit(hit: SearchHit) {
  activeSearchHitId.value = hit.message_id;
  activeId.value = hit.conversation_id;
}

function formatHitTime(timestamp: number | null) {
  if (!timestamp) return "";
  return formatDateTime(timestamp, locale.value as AppLocale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function searchHitRoleLabel(hit: SearchHit) {
  if (hit.role === "user") return t("common.you");
  if (hit.role === "assistant") return sourceLabel(hit.source ?? "chatgpt");
  if (hit.role === "system") {
    const src = hit.source?.toLowerCase();
    if (src === "cursor" || src === "codex") return sourceLabel(src);
    return t("common.system");
  }
  return hit.role;
}

async function toggleConversationStar() {
  if (!displayConversation.value) return;
  const next = !displayConversation.value.is_starred;
  await setConversationStarred(displayConversation.value.id, next);
  if (activeConversation.value) {
    activeConversation.value.is_starred = next;
  } else if (fetchedConversation.value) {
    fetchedConversation.value.is_starred = next;
  }
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
  if (starredMessagesMode.value) {
    await loadStarredMessages();
    if (!starred && activeSearchHitId.value === messageId) {
      activeSearchHitId.value = null;
    }
  }
}

async function handleExportMarkdown() {
  if (!displayConversation.value) return;

  const defaultName = `${displayConversation.value.title.replace(/[\\/:*?"<>|]/g, "_")}.md`;
  const outputPath = await save({
    defaultPath: defaultName,
    filters: [{ name: "Markdown", extensions: ["md"] }],
    title: t("conversation.exportDialogTitle"),
  });

  if (!outputPath) return;

  exporting.value = true;
  try {
    const result = await exportConversationMarkdown(
      displayConversation.value.id,
      outputPath,
    );
    ElMessage.success(t("conversation.exportSuccess", { count: result.message_count }));
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
    ElMessage.success(t("tags.created", { name }));
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
    ElMessage.success(t("tags.deleted"));
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function handleSaveTags(tagIds: number[]) {
  if (!displayConversation.value) return;
  try {
    const tagNames = await setConversationTags(displayConversation.value.id, tagIds);
    if (activeConversation.value) {
      activeConversation.value.tags = tagNames;
    } else if (fetchedConversation.value) {
      fetchedConversation.value.tags = tagNames;
    }
    await refreshTags();
    await loadConversations();
    ElMessage.success(t("tags.updated"));
  } catch (error) {
    ElMessage.error(String(error));
  }
}

watch(activeId, async (id) => {
  conversationInfoDrawerVisible.value = false;
  if (id) {
    await Promise.all([loadMessages(id), ensureConversationSummary(id)]);
  } else {
    messages.value = [];
    fetchedConversation.value = null;
  }
});

watch(showRightPanel, (visible) => {
  if (visible) {
    conversationInfoDrawerVisible.value = false;
  }
});

watch(viewMode, () => {
  conversationInfoDrawerVisible.value = false;
});

watch(
  [listQuery, starredOnly, filterHasImages, filterHasCode, filterHasAttachments, filterTagId, filterSource],
  async () => {
  if (starredMessagesMode.value) {
    await loadStarredMessages();
    return;
  }
  if (!searchMode.value) {
    await loadConversations();
  }
});

onMounted(async () => {
  removeKeyboardShortcuts = installDesktopBehaviors();
  await refreshStats();
  await refreshTags();
  await loadConversations();
  activeId.value = conversations.value[0]?.id ?? null;
});
</script>

<template>
  <el-config-provider :locale="elementLocale">
  <el-container class="app-shell">
    <el-header class="topbar titlebar" :height="`${48}px`">
      <div class="titlebar-left">
        <div class="brand" data-tauri-drag-region>
          <strong class="brand-name" data-tauri-drag-region>Chatilus</strong>
        </div>
        <nav class="nav-tabs" :aria-label="t('nav.mainViews')">
          <button
            v-for="item in navItems"
            :key="item.id"
            class="nav-tab"
            :class="{ active: viewMode === item.id }"
            type="button"
            @click="handleNavClick(item.id)"
          >
            <span class="nav-label">{{ item.label }}</span>
          </button>
        </nav>
      </div>
      <div class="titlebar-drag" data-tauri-drag-region aria-hidden="true" />
      <div class="titlebar-right">
        <el-input
          ref="searchInputRef"
          v-model="searchQuery"
          clearable
          :placeholder="t('search.placeholder')"
          class="search-input"
          @keyup.enter="handleSearch"
          @clear="clearSearch"
        >
          <template #prefix>
            <span class="search-prefix" aria-hidden="true">⌕</span>
          </template>
        </el-input>
        <el-button class="toolbar-btn" :loading="importing" @click="openImportWizard">
          {{ t("common.import") }}
        </el-button>
        <el-dropdown trigger="click" @command="handleMoreCommand">
          <el-button class="toolbar-btn toolbar-more" :title="t('common.more')">
            ⋯
          </el-button>
          <template #dropdown>
            <el-dropdown-menu class="more-menu">
              <el-dropdown-item disabled class="menu-stats">
                <span class="menu-stats-label">{{ t("library.overview") }}</span>
                <span class="menu-stats-value">{{ librarySummaryLine }}</span>
              </el-dropdown-item>
              <el-dropdown-item disabled divided class="menu-section">
                {{ t("language.title") }}
              </el-dropdown-item>
              <el-dropdown-item
                v-for="option in languageOptions"
                :key="option.value"
                :command="`lang:${option.value}`"
              >
                <span
                  class="menu-check-item"
                  :class="{ selected: locale === option.value }"
                >
                  {{ option.label }}
                </span>
              </el-dropdown-item>
              <el-dropdown-item disabled divided class="menu-section">
                {{ t("appearance.title") }}
              </el-dropdown-item>
              <el-dropdown-item
                v-for="option in appearanceOptions"
                :key="option.value"
                :command="`appearance:${option.value}`"
              >
                <span
                  class="menu-check-item"
                  :class="{ selected: appearanceMode === option.value }"
                >
                  {{ option.label }}
                </span>
              </el-dropdown-item>
              <el-dropdown-item divided command="privacy">
                {{ t("privacy.menu") }}
              </el-dropdown-item>
              <el-dropdown-item command="feedback">
                {{ t("feedback.menu") }}
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <WindowControls />
      </div>
    </el-header>

    <el-dialog
      v-model="importWizardVisible"
      :title="importWizardTitle"
      width="520px"
      destroy-on-close
    >
      <div v-if="importWizardStep === 'source'" class="import-source-grid">
        <button
          v-for="guide in importGuides"
          :key="guide.importer_id"
          type="button"
          class="import-source-card"
          @click="selectImportSource(guide)"
        >
          <div class="import-source-card-header">
            <el-tag size="small" :type="sourceTagType(guide.source)">
              {{ sourceLabel(guide.source) }}
            </el-tag>
            <el-tag
              size="small"
              :type="importSupportStatusType(guide.support_status)"
              effect="plain"
            >
              {{ importSupportStatusLabel(guide.support_status) }}
            </el-tag>
          </div>
          <strong class="import-source-name">{{ guide.display_name }}</strong>
          <p class="import-source-summary">{{ guide.support_summary }}</p>
          <p class="import-source-desc">{{ guide.description }}</p>
        </button>
      </div>

      <div v-else-if="selectedImportGuide" class="import-method-panel">
        <p class="import-method-desc">{{ selectedImportGuide.description }}</p>
        <p class="import-recognition-hint">
          <span class="import-recognition-label">{{ t("import.recognitionBasis") }}</span>
          {{ selectedImportGuide.recognition_hint }}
        </p>
        <div
          v-for="method in selectedImportGuide.methods"
          :key="method.id"
          class="import-method-card"
        >
          <div
            v-if="method.detected_default_path"
            class="import-detected-default"
          >
            <p class="import-detected-label">
              {{ method.detected_default_label ?? t("import.detectedDefaultPath") }}
            </p>
            <code class="import-method-example">{{ method.detected_default_path }}</code>
            <div class="import-detected-actions">
              <el-button
                type="primary"
                size="small"
                @click="importFromDetectedPath(method.detected_default_path!)"
              >
                {{ t("import.importDirectly") }}
              </el-button>
              <el-button size="small" @click="pickImportPath(method)">
                {{ t("import.chooseManually") }}
              </el-button>
            </div>
          </div>
          <template v-else>
            <button
              type="button"
              class="import-method-action"
              @click="pickImportPath(method)"
            >
              {{ method.label }}
            </button>
            <p class="import-method-hint">{{ method.hint }}</p>
            <code v-if="method.example_path" class="import-method-example">
              {{ method.example_path }}
            </code>
          </template>
        </div>
        <el-button class="import-wizard-back" @click="backToImportSources">
          {{ t("import.backToSources") }}
        </el-button>
      </div>
    </el-dialog>

    <el-dialog
      v-model="importDialogVisible"
      :title="t('import.inProgress')"
      width="420px"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
      :show-close="false"
    >
      <p>{{ importPhaseLabel }}</p>
      <p class="import-progress-elapsed">
        {{ formatImportElapsed(importElapsedMs) }}
      </p>
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
      <el-aside
        class="sidebar"
        :class="{ compact: compactLeftPanel }"
        :style="{
          width: compactLeftPanel
            ? 'var(--cl-sidebar-width-compact)'
            : 'var(--cl-sidebar-width)',
        }"
      >
        <div v-if="!searchMode && !starredMessagesMode" class="sidebar-controls">
          <SourceNav
            :items="visibleSourceNavItems"
            :active-id="filterSource"
            :title="t('filter.sources')"
            show-dots
            @select="filterSource = $event"
          />

          <div class="sidebar-section">
            <div class="sidebar-section-title">{{ t("filter.filters") }}</div>
          <el-input
            v-model="listQuery"
            clearable
            size="small"
            :placeholder="t('filter.filterTitle')"
          />

          <div class="filter-row">
            <div class="filter-chips">
              <button
                type="button"
                class="filter-chip"
                :class="{ active: starredOnly }"
                @click="toggleStarredOnlyFilter"
              >
                {{ t("filter.starredConversations") }}
              </button>
              <button
                type="button"
                class="filter-chip"
                :class="{ active: starredMessagesMode }"
                @click="toggleStarredMessagesFilter"
              >
                {{ t("filter.starredMessages") }}
              </button>
              <button
                type="button"
                class="filter-chip"
                :class="{ active: filterHasImages }"
                @click="filterHasImages = !filterHasImages"
              >
                {{ t("filter.hasImages") }}
              </button>
              <button
                type="button"
                class="filter-chip"
                :class="{ active: filterHasCode }"
                @click="filterHasCode = !filterHasCode"
              >
                {{ t("filter.hasCode") }}
              </button>
              <button
                type="button"
                class="filter-chip"
                :class="{ active: filterHasAttachments }"
                @click="filterHasAttachments = !filterHasAttachments"
              >
                {{ t("filter.hasAttachments") }}
              </button>
            </div>
            <el-select
              v-if="tags.length > 0"
              v-model="filterTagId"
              clearable
              size="small"
              :placeholder="t('filter.tags')"
              class="tag-filter-inline"
            >
              <el-option
                v-for="tag in tags"
                :key="tag.id"
                :label="`${tag.name} (${tag.conversation_count})`"
                :value="tag.id"
              />
            </el-select>
          </div>
          </div>
        </div>

        <div v-else-if="!searchMode && starredMessagesMode" class="sidebar-controls sidebar-controls-compact">
          <SourceNav
            :items="visibleSourceNavItems"
            :active-id="filterSource"
            :title="t('filter.sources')"
            show-dots
            @select="filterSource = $event"
          />
          <button
            type="button"
            class="filter-chip filter-chip-wide active"
            @click="toggleStarredMessagesFilter"
          >
            {{ t("filter.clearStarredMessages") }}
          </button>
        </div>

        <div v-if="searchMode" class="search-results">
          <div class="section-title">
            {{ t("search.results") }}
            <span v-if="searchHits.length > 0" class="search-count">
              {{ t("search.hits", { count: searchHits.length }) }}
            </span>
          </div>
          <el-empty v-if="searchHits.length === 0" :description="t('search.noHits')" />
          <button
            v-for="hit in searchHits"
            :key="hit.message_id"
            class="search-hit"
            :class="{ 'cl-nav-item-active': hit.message_id === activeSearchHitId }"
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

        <div v-else-if="starredMessagesMode" class="search-results">
          <div class="section-title">
            {{ t("filter.starredMessages") }}
            <span v-if="starredMessageHits.length > 0" class="search-count">
              {{ t("search.hits", { count: starredMessageHits.length }) }}
            </span>
          </div>
          <el-skeleton v-if="starredMessagesLoading" animated :rows="6" />
          <el-empty
            v-else-if="starredMessageHits.length === 0"
            :description="t('starred.empty')"
          />
          <template v-else>
            <button
              v-for="hit in starredMessageHits"
              :key="hit.message_id"
              class="search-hit"
              :class="{ 'cl-nav-item-active': hit.message_id === activeSearchHitId }"
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
              <div class="hit-snippet">{{ hit.snippet }}</div>
            </button>
          </template>
        </div>

        <div v-else-if="!searchMode && !starredMessagesMode" class="sidebar-list-panel">
          <div class="sidebar-section-title">{{ t("filter.conversations") }}</div>
          <ConversationList
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
        </div>
      </el-aside>

      <el-main class="main">
        <div class="main-layout">
          <MessageView
            :messages="messages"
            :title="displayConversationTitle"
            :data-source="displayConversationSource"
            :loading="messageLoading || exporting"
            :conversation-starred="displayConversation?.is_starred ?? false"
            :conversation-tags="displayConversation?.tags ?? []"
            :highlight-message-id="activeSearchHitId"
            :show-info-button="!showRightPanel && !!displayConversation"
            @toggle-conversation-star="toggleConversationStar"
            @export-markdown="handleExportMarkdown"
            @toggle-message-star="toggleMessageStar"
            @open-conversation-info="conversationInfoDrawerVisible = true"
          />
          <ConversationInfo
            v-if="displayConversation && showRightPanel"
            :conversation="displayConversation"
            :messages="messages"
          />
        </div>
        <div v-if="displayConversation" class="tag-fab">
          <el-button size="small" @click="tagDialogVisible = true">{{ t("tags.manage") }}</el-button>
        </div>
      </el-main>
    </el-container>

    <el-main v-else-if="viewMode === 'timeline'" class="main gallery-main">
      <TimelineView
        :key="`timeline:${stats?.conversation_count ?? 0}:${filterSource ?? 'all'}`"
        v-model:filter-source="filterSource"
        :total-count="stats?.conversation_count ?? null"
        :conversation-counts-by-source="stats?.conversation_counts_by_source ?? []"
        @open-conversation="openConversationFromTimeline"
        @open-message="openConversationFromTimeline"
        @open-gallery="openGalleryFromTimeline"
      />
    </el-main>

    <el-main v-else class="main gallery-main">
      <ImageGallery
        :key="`${stats?.image_count ?? 0}:${filterSource ?? 'all'}:${galleryMonth ?? ''}:${galleryConversationId ?? ''}`"
        v-model:filter-source="filterSource"
        :filter-month="galleryMonth"
        :filter-conversation-id="galleryConversationId"
        :total-count="stats?.image_count ?? null"
        :generated-count="stats?.generated_image_count ?? null"
        :upload-count="stats?.upload_image_count ?? null"
        :image-counts-by-source="stats?.image_counts_by_source ?? []"
        @open-conversation="openConversationFromGallery"
      />
    </el-main>

    <el-drawer
      v-if="viewMode === 'chats'"
      v-model="conversationInfoDrawerVisible"
      :title="t('conversation.info')"
      direction="rtl"
      :size="insightPanelWidth"
      append-to-body
    >
      <ConversationInfo
        v-if="displayConversation"
        variant="drawer"
        :conversation="displayConversation"
        :messages="messages"
      />
    </el-drawer>

    <TagDialog
      v-model:visible="tagDialogVisible"
      :tags="tags"
      :selected-tag-ids="activeTagIds"
      @save="handleSaveTags"
      @create-tag="handleCreateTag"
      @delete-tag="handleDeleteTag"
    />

    <ImportReportDialog
      v-model:visible="importReportVisible"
      :job="lastImportJob"
      @open-timeline="handleImportReportOpenTimeline"
      @start-search="handleImportReportStartSearch"
    />

    <PrivacyDataDialog
      v-model:visible="privacyDialogVisible"
      :db-path="stats?.db_path"
    />
  </el-container>
  </el-config-provider>
</template>

<style scoped>
.app-shell {
  height: 100vh;
  height: 100dvh;
  background: var(--cl-bg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.topbar {
  display: flex;
  align-items: stretch;
  justify-content: space-between;
  gap: 12px;
  padding: 0 0 0 16px;
  min-height: var(--cl-toolbar-height);
  border-bottom: 1px solid var(--cl-border-subtle);
  background: var(--cl-panel-elevated);
  flex-shrink: 0;
}

.titlebar-left {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
  flex-shrink: 0;
}

.titlebar-drag {
  flex: 1;
  min-width: 24px;
  align-self: stretch;
}

.titlebar-right {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
  padding-right: 0;
}

.brand {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.brand-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--cl-text);
  letter-spacing: -0.02em;
  flex-shrink: 0;
}

.nav-tabs {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: 8px;
  background: var(--cl-bg);
  border: 1px solid var(--cl-border-subtle);
}

.nav-tab {
  border: none;
  background: transparent;
  padding: 5px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--cl-text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  transition: background 0.15s ease, color 0.15s ease;
}

.nav-tab:hover {
  color: var(--cl-text);
  background: var(--cl-hover);
}

.nav-tab.active {
  background: var(--cl-panel-elevated);
  color: var(--cl-text);
  font-weight: 500;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
}

.nav-label {
  line-height: 1.2;
}

.actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.sidebar-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.sidebar-section-title {
  padding: 4px 12px 0;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--cl-text-faint);
}

.sidebar-section-title-list {
  padding: 10px 12px 4px;
  flex-shrink: 0;
}

.sidebar-list-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.search-input {
  width: 260px;
}

.search-prefix {
  color: var(--cl-text-faint);
  font-size: 15px;
  line-height: 1;
}

.toolbar-btn {
  --el-button-bg-color: transparent;
  --el-button-border-color: var(--cl-border);
  --el-button-text-color: var(--cl-text-muted);
  --el-button-hover-bg-color: var(--cl-hover);
  --el-button-hover-border-color: var(--cl-border);
  --el-button-hover-text-color: var(--cl-text);
}

.toolbar-more {
  min-width: 36px;
  padding-inline: 10px;
  font-size: 16px;
  letter-spacing: 0.08em;
}

:deep(.more-menu .menu-stats) {
  display: flex;
  flex-direction: column;
  gap: 2px;
  line-height: 1.35;
  cursor: default;
  opacity: 1;
}

.menu-stats-label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--cl-text-faint);
}

.menu-stats-value {
  font-size: 12px;
  color: var(--cl-text-muted);
}

:deep(.more-menu .menu-section) {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--cl-text-faint);
  cursor: default;
  opacity: 1;
}

.menu-check-item {
  display: inline-flex;
  align-items: center;
  min-width: 120px;
}

.menu-check-item::before {
  content: "";
  display: inline-block;
  width: 1em;
  margin-right: 6px;
}

.menu-check-item.selected {
  color: var(--cl-text);
  font-weight: 500;
}

.menu-check-item.selected::before {
  content: "✓";
}

.body {
  min-height: 0;
  flex: 1;
}

.sidebar {
  width: var(--cl-sidebar-width);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--cl-border-subtle);
  background: var(--cl-panel);
  min-height: 0;
  overflow: hidden;
}

.sidebar.compact .sidebar-controls {
  padding-inline: 6px;
}

.sidebar.compact .sidebar-section-title {
  padding-inline: 8px;
}

.sidebar-controls {
  padding: 8px 8px 10px;
  border-bottom: 1px solid var(--cl-border-subtle);
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.filter-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.filter-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.sidebar-controls-compact {
  gap: 10px;
}

.filter-chip-wide {
  align-self: flex-start;
}

.filter-chip {
  border: 1px solid var(--cl-border-subtle);
  background: transparent;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  color: var(--cl-text-muted);
  cursor: pointer;
  line-height: 1.5;
}

.filter-chip:hover {
  background: var(--cl-hover);
  color: var(--cl-text);
}

.filter-chip.active {
  border-color: var(--cl-border);
  background: var(--cl-selected-strong);
  color: var(--cl-text);
  font-weight: 500;
}

.tag-filter-inline {
  width: 96px;
  flex-shrink: 0;
}

.sidebar > .conversation-list,
.sidebar > .search-results {
  flex: 1;
  min-height: 0;
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

.search-hit:hover:not(.cl-nav-item-active) {
  background: var(--cl-accent-soft);
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
  min-width: var(--cl-main-min-width);
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

.import-progress-elapsed,
.import-progress-detail {
  margin-top: 8px;
  font-size: 12px;
  color: var(--cl-text-muted);
}

.import-progress-elapsed {
  margin-top: 4px;
  font-variant-numeric: tabular-nums;
}

.import-source-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.import-source-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 14px;
  border: 1px solid var(--cl-border);
  border-radius: 10px;
  background: var(--cl-surface);
  text-align: left;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}

.import-source-card-header {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  width: 100%;
}

.import-source-summary {
  margin: 0;
  font-size: 12px;
  font-weight: 600;
  color: var(--cl-text);
}

.import-source-card:hover {
  border-color: var(--el-color-primary);
  background: var(--cl-surface-raised, var(--cl-surface));
}

.import-source-name {
  font-size: 15px;
  color: var(--cl-text);
}

.import-source-desc {
  margin: 0;
  font-size: 12px;
  line-height: 1.45;
  color: var(--cl-text-muted);
}

.import-method-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.import-method-desc {
  margin: 0 0 4px;
  font-size: 13px;
  color: var(--cl-text-muted);
}

.import-recognition-hint {
  margin: 0 0 12px;
  padding: 10px 12px;
  border-radius: 8px;
  background: var(--cl-code-bg, rgba(127, 127, 127, 0.08));
  font-size: 12px;
  line-height: 1.5;
  color: var(--cl-text-muted);
}

.import-recognition-label {
  display: block;
  margin-bottom: 4px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: var(--cl-text);
}

.import-detected-default {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.import-detected-label {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text);
}

.import-detected-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.import-method-card {
  padding: 12px 14px;
  border: 1px solid var(--cl-border);
  border-radius: 10px;
  background: var(--cl-surface);
}

.import-method-action {
  display: block;
  width: 100%;
  padding: 0;
  border: none;
  background: none;
  font-size: 14px;
  font-weight: 600;
  color: var(--el-color-primary);
  text-align: left;
  cursor: pointer;
}

.import-method-action:hover {
  text-decoration: underline;
}

.import-method-hint {
  margin: 8px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--cl-text-muted);
  white-space: pre-wrap;
}

.import-method-example {
  display: block;
  margin-top: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--cl-code-bg, rgba(127, 127, 127, 0.12));
  font-size: 11px;
  color: var(--cl-text);
  word-break: break-all;
}

.import-wizard-back {
  align-self: flex-start;
  margin-top: 8px;
}
</style>
