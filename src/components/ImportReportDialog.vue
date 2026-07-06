<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ElMessage } from "element-plus";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { ImportJobView } from "../types";
import { sourceLabel } from "../utils/dataSource";

const FEEDBACK_URL = "https://github.com/manxisuo/ChatLens/issues";

const props = defineProps<{
  visible: boolean;
  job: ImportJobView | null;
}>();

const emit = defineEmits<{
  "update:visible": [value: boolean];
  openTimeline: [];
  startSearch: [];
}>();

const { t } = useI18n();

const isSuccess = computed(() => props.job?.status === "done" && !!props.job?.result);

const sourceName = computed(() => {
  const source = props.job?.source;
  if (source) return sourceLabel(source);
  return props.job?.export_label ?? t("import.report.unknownSource");
});

async function copyError() {
  const text = props.job?.error;
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    ElMessage.success(t("import.report.errorCopied"));
  } catch {
    ElMessage.error(t("import.report.copyFailed"));
  }
}

function close() {
  emit("update:visible", false);
}

function onOpenTimeline() {
  emit("openTimeline");
  close();
}

function onStartSearch() {
  emit("startSearch");
  close();
}

function openFeedback() {
  void openUrl(FEEDBACK_URL);
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :title="isSuccess ? t('import.report.successTitle') : t('import.report.failureTitle')"
    width="460px"
    @update:model-value="emit('update:visible', $event)"
  >
    <template v-if="isSuccess && job?.result">
      <p class="report-source">
        {{ t("import.report.source", { name: sourceName }) }}
      </p>
      <dl class="report-stats">
        <div class="report-row">
          <dt>{{ t("import.report.importedLabel") }}</dt>
          <dd>{{ job.result.conversations_imported }}</dd>
        </div>
        <div class="report-row">
          <dt>{{ t("import.report.updatedLabel") }}</dt>
          <dd>{{ job.result.conversations_updated }}</dd>
        </div>
        <div class="report-row">
          <dt>{{ t("import.report.deduplicatedLabel") }}</dt>
          <dd>{{ job.result.conversations_deduplicated }}</dd>
        </div>
        <div class="report-row">
          <dt>{{ t("import.report.messagesLabel") }}</dt>
          <dd>{{ job.result.messages_imported }}</dd>
        </div>
        <div class="report-row">
          <dt>{{ t("import.report.mediaLabel") }}</dt>
          <dd>{{ job.result.media_files_indexed }}</dd>
        </div>
      </dl>
      <p
        v-if="
          job.result.conversations_imported === 0 &&
          job.result.conversations_updated === 0
        "
        class="report-hint"
      >
        {{ t("import.result.noneFound") }}
      </p>
    </template>

    <template v-else>
      <p class="report-error">{{ job?.error ?? t("import.failed") }}</p>
      <div v-if="job?.error" class="report-error-actions">
        <el-button text @click="copyError">{{ t("import.report.copyError") }}</el-button>
        <el-button text @click="openFeedback">{{ t("import.report.feedback") }}</el-button>
      </div>
    </template>

    <template #footer>
      <template v-if="isSuccess">
        <el-button @click="close">{{ t("common.close") }}</el-button>
        <el-button @click="onOpenTimeline">{{ t("import.report.openTimeline") }}</el-button>
        <el-button type="primary" @click="onStartSearch">
          {{ t("import.report.startSearch") }}
        </el-button>
      </template>
      <template v-else>
        <el-button type="primary" @click="close">{{ t("common.close") }}</el-button>
      </template>
    </template>
  </el-dialog>
</template>

<style scoped>
.report-source {
  margin: 0 0 12px;
  font-size: 13px;
  color: var(--cl-text-muted);
}

.report-stats {
  margin: 0;
}

.report-row {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding: 8px 0;
  border-bottom: 1px solid var(--cl-border-subtle);
  font-size: 13px;
}

.report-row:last-child {
  border-bottom: none;
}

.report-row dt {
  margin: 0;
  color: var(--cl-text-muted);
}

.report-row dd {
  margin: 0;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  color: var(--cl-text);
}

.report-hint {
  margin: 12px 0 0;
  font-size: 12px;
  color: var(--cl-text-faint);
}

.report-error {
  margin: 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--cl-text);
  word-break: break-word;
  white-space: pre-wrap;
}

.report-error-actions {
  display: flex;
  gap: 4px;
  margin-top: 8px;
}
</style>
