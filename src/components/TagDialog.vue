<script setup lang="ts">
import { ref, watch } from "vue";
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
    title="管理标签"
    width="420px"
    @update:model-value="emit('update:visible', $event)"
  >
    <div class="section">
      <div class="section-title">为当前对话选择标签</div>
      <el-checkbox-group v-model="draftTagIds" class="tag-list">
        <el-checkbox v-for="tag in tags" :key="tag.id" :label="tag.id">
          {{ tag.name }}
          <span class="count">({{ tag.conversation_count }})</span>
        </el-checkbox>
      </el-checkbox-group>
      <el-empty v-if="tags.length === 0" description="还没有标签" />
    </div>

    <div class="section">
      <div class="section-title">新建标签</div>
      <div class="create-row">
        <el-input v-model="newTagName" placeholder="输入标签名" @keyup.enter="createTag" />
        <el-button @click="createTag">添加</el-button>
      </div>
    </div>

    <div v-if="tags.length" class="section">
      <div class="section-title">删除标签</div>
      <div class="delete-list">
        <div v-for="tag in tags" :key="tag.id" class="delete-item">
          <span>{{ tag.name }}</span>
          <el-button
            type="danger"
            text
            size="small"
            @click="emit('deleteTag', tag.id)"
          >
            删除
          </el-button>
        </div>
      </div>
    </div>

    <template #footer>
      <el-button @click="close">取消</el-button>
      <el-button type="primary" @click="save">保存</el-button>
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
