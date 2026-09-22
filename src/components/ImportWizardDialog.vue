<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { ImportGuide, ImportMethodGuide } from "../types";
import { sourceLabel, sourceTagType } from "../utils/dataSource";

const { t } = useI18n();

const props = defineProps<{
  wizardVisible: boolean;
  wizardTitle: string;
  wizardStep: "source" | "method";
  importGuides: ImportGuide[];
  selectedImportGuide: ImportGuide | null;
  progressVisible: boolean;
  phaseLabel: string;
  elapsedLabel: string;
  progressPercent: number;
  progressProcessed: number;
  progressTotal: number;
  supportStatusLabel: (status: ImportGuide["support_status"]) => string;
  supportStatusType: (
    status: ImportGuide["support_status"],
  ) => "success" | "warning" | "info";
}>();

const emit = defineEmits<{
  "update:wizardVisible": [value: boolean];
  "update:progressVisible": [value: boolean];
  selectSource: [guide: ImportGuide];
  back: [];
  pickPath: [method: ImportMethodGuide];
  importDetected: [path: string];
}>();
</script>

<template>
  <el-dialog
    :model-value="props.wizardVisible"
    :title="props.wizardTitle"
    width="520px"
    destroy-on-close
    @update:model-value="emit('update:wizardVisible', $event)"
  >
    <div v-if="props.wizardStep === 'source'" class="import-source-grid">
      <button
        v-for="guide in props.importGuides"
        :key="guide.importer_id"
        type="button"
        class="import-source-card"
        @click="emit('selectSource', guide)"
      >
        <div class="import-source-card-header">
          <el-tag size="small" :type="sourceTagType(guide.source)">
            {{ sourceLabel(guide.source) }}
          </el-tag>
          <el-tag
            size="small"
            :type="props.supportStatusType(guide.support_status)"
            effect="plain"
          >
            {{ props.supportStatusLabel(guide.support_status) }}
          </el-tag>
        </div>
        <strong class="import-source-name">{{ guide.display_name }}</strong>
        <p class="import-source-summary">{{ guide.support_summary }}</p>
        <p class="import-source-desc">{{ guide.description }}</p>
      </button>
    </div>

    <div v-else-if="props.selectedImportGuide" class="import-method-panel">
      <p class="import-method-desc">{{ props.selectedImportGuide.description }}</p>
      <p class="import-recognition-hint">
        <span class="import-recognition-label">{{ t("import.recognitionBasis") }}</span>
        {{ props.selectedImportGuide.recognition_hint }}
      </p>
      <div
        v-for="method in props.selectedImportGuide.methods"
        :key="method.id"
        class="import-method-card"
      >
        <div v-if="method.detected_default_path" class="import-detected-default">
          <p class="import-detected-label">
            {{ method.detected_default_label ?? t("import.detectedDefaultPath") }}
          </p>
          <code class="import-method-example">{{ method.detected_default_path }}</code>
          <div class="import-detected-actions">
            <el-button
              type="primary"
              size="small"
              @click="emit('importDetected', method.detected_default_path!)"
            >
              {{ t("import.importDirectly") }}
            </el-button>
            <el-button size="small" @click="emit('pickPath', method)">
              {{ t("import.chooseManually") }}
            </el-button>
          </div>
        </div>
        <template v-else>
          <button
            type="button"
            class="import-method-action"
            @click="emit('pickPath', method)"
          >
            {{ method.label }}
          </button>
          <p class="import-method-hint">{{ method.hint }}</p>
          <code v-if="method.example_path" class="import-method-example">
            {{ method.example_path }}
          </code>
        </template>
      </div>
      <el-button class="import-wizard-back" @click="emit('back')">
        {{ t("import.backToSources") }}
      </el-button>
    </div>
  </el-dialog>

  <el-dialog
    :model-value="props.progressVisible"
    :title="t('import.inProgress')"
    width="420px"
    :close-on-click-modal="false"
    :close-on-press-escape="false"
    :show-close="false"
    @update:model-value="emit('update:progressVisible', $event)"
  >
    <p>{{ props.phaseLabel }}</p>
    <p class="import-progress-elapsed">{{ props.elapsedLabel }}</p>
    <el-progress
      :percentage="props.progressPercent"
      :stroke-width="16"
      striped
      striped-flow
    />
    <p v-if="props.progressTotal > 0" class="import-progress-detail">
      {{ props.progressProcessed }} / {{ props.progressTotal }}
    </p>
  </el-dialog>
</template>

<style scoped>
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
