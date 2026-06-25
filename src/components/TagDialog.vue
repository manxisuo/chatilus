<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { TagView } from "../types";

const props = defineProps<{
  visible: boolean;
  tags: TagView[];
  selectedTagIds: number[];
}>();

const emit = defineEmits<{
  "update:visible": [value: boolean];
  save: [tagIds: number[]];
  createTag: [name: string];
  deleteTag: [tagId: number];
}>();

const { t } = useI18n();

const draftTagIds = ref<number[]>([]);
const newTagName = ref("");

watch(
  () => props.visible,
  (open) => {
    if (open) {
      draftTagIds.value = [...props.selectedTagIds];
      newTagName.value = "";
    }
  },
);

function close() {
  emit("update:visible", false);
}

function save() {
  emit("save", draftTagIds.value);
  close();
}

function createTag() {
  const name = newTagName.value.trim();
  if (!name) return;
  emit("createTag", name);
  newTagName.value = "";
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :title="t('tags.manage')"
    width="420px"
    @update:model-value="emit('update:visible', $event)"
  >
    <div class="section">
      <div class="section-title">{{ t("tags.selectForConversation") }}</div>
      <el-checkbox-group v-model="draftTagIds" class="tag-list">
        <el-checkbox v-for="tag in tags" :key="tag.id" :label="tag.id">
          {{ tag.name }}
          <span class="count">({{ tag.conversation_count }})</span>
        </el-checkbox>
      </el-checkbox-group>
      <el-empty v-if="tags.length === 0" :description="t('tags.empty')" />
    </div>

    <div class="section">
      <div class="section-title">{{ t("tags.create") }}</div>
      <div class="create-row">
        <el-input
          v-model="newTagName"
          :placeholder="t('tags.namePlaceholder')"
          @keyup.enter="createTag"
        />
        <el-button @click="createTag">{{ t("common.add") }}</el-button>
      </div>
    </div>

    <div v-if="tags.length" class="section">
      <div class="section-title">{{ t("tags.deleteSection") }}</div>
      <div class="delete-list">
        <div v-for="tag in tags" :key="tag.id" class="delete-item">
          <span>{{ tag.name }}</span>
          <el-button
            type="danger"
            text
            size="small"
            @click="emit('deleteTag', tag.id)"
          >
            {{ t("common.delete") }}
          </el-button>
        </div>
      </div>
    </div>

    <template #footer>
      <el-button @click="close">{{ t("common.cancel") }}</el-button>
      <el-button type="primary" @click="save">{{ t("common.save") }}</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.section + .section {
  margin-top: 20px;
}

.section-title {
  margin-bottom: 10px;
  font-size: 13px;
  font-weight: 600;
  color: var(--cl-text);
}

.tag-list {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
}

.count {
  color: var(--cl-text-muted);
  font-size: 12px;
}

.create-row {
  display: flex;
  gap: 8px;
}

.delete-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.delete-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 0;
  border-bottom: 1px solid var(--cl-border);
}
</style>
