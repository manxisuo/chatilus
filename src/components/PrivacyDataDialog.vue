<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";

const FEEDBACK_URL = "https://github.com/manxisuo/chatilus/issues";
const REPO_URL = "https://github.com/manxisuo/chatilus";

const PRIVACY_SECTIONS = ["local", "noNetwork", "storage", "importCache", "delete"] as const;

const props = defineProps<{
  visible: boolean;
  dbPath?: string | null;
}>();

const emit = defineEmits<{
  "update:visible": [value: boolean];
}>();

const { t } = useI18n();
const appVersion = ref("");

async function loadVersion() {
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = "";
  }
}

onMounted(loadVersion);

watch(
  () => props.visible,
  (open) => {
    if (open) void loadVersion();
  },
);

function openFeedback() {
  void openUrl(FEEDBACK_URL);
}

function openRepo() {
  void openUrl(REPO_URL);
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :title="t('privacy.title')"
    width="520px"
    @update:model-value="emit('update:visible', $event)"
  >
    <p v-if="appVersion" class="privacy-version">
      {{ t("privacy.version", { version: appVersion }) }}
    </p>

    <section
      v-for="key in PRIVACY_SECTIONS"
      :key="key"
      class="privacy-section"
    >
      <h3>{{ t(`privacy.sections.${key}.title`) }}</h3>
      <p>{{ t(`privacy.sections.${key}.body`) }}</p>
    </section>

    <p v-if="dbPath" class="privacy-path">
      <span class="privacy-path-label">{{ t("privacy.dbPath") }}</span>
      <code>{{ dbPath }}</code>
    </p>

    <template #footer>
      <el-button text @click="openRepo">{{ t("privacy.viewRepo") }}</el-button>
      <el-button text @click="openFeedback">{{ t("privacy.feedback") }}</el-button>
      <el-button type="primary" @click="emit('update:visible', false)">
        {{ t("common.close") }}
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.privacy-version {
  margin: 0 0 16px;
  font-size: 12px;
  color: var(--cl-text-faint);
}

.privacy-section {
  margin-bottom: 16px;
}

.privacy-section h3 {
  margin: 0 0 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text);
}

.privacy-section p {
  margin: 0;
  font-size: 13px;
  line-height: 1.55;
  color: var(--cl-text-muted);
}

.privacy-path {
  margin: 8px 0 0;
  padding: 10px 12px;
  border-radius: 8px;
  background: var(--cl-selected);
  font-size: 12px;
}

.privacy-path-label {
  display: block;
  margin-bottom: 6px;
  color: var(--cl-text-faint);
}

.privacy-path code {
  display: block;
  word-break: break-all;
  font-family: ui-monospace, Consolas, monospace;
  color: var(--cl-text);
}
</style>
