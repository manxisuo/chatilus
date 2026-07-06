<script setup lang="ts">
import { sourceAccentColor } from "../utils/dataSource";

defineProps<{
  items: Array<{ id: string | null; label: string; count: number }>;
  activeId: string | null;
  title?: string;
  showDots?: boolean;
}>();

const emit = defineEmits<{
  select: [id: string | null];
}>();
</script>

<template>
  <nav class="source-nav-list" :aria-label="title">
    <div v-if="title" class="source-nav-title">{{ title }}</div>
    <button
      v-for="item in items"
      :key="item.id ?? 'all'"
      type="button"
      class="source-nav-row"
      :class="{ 'cl-nav-item-active': activeId === item.id }"
      @click="emit('select', item.id)"
    >
      <span
        v-if="showDots && item.id"
        class="source-dot"
        :style="{ background: sourceAccentColor(item.id) }"
        aria-hidden="true"
      />
      <span v-else-if="showDots" class="source-dot source-dot-all" aria-hidden="true" />
      <span class="source-nav-label">{{ item.label }}</span>
      <span class="source-nav-count">{{ item.count.toLocaleString() }}</span>
    </button>
  </nav>
</template>

<style scoped>
.source-nav-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.source-nav-title {
  padding: 10px 12px 6px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--cl-text-faint);
}

.source-nav-row {
  width: 100%;
  border: none;
  background: transparent;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--cl-text);
  cursor: pointer;
  text-align: left;
  line-height: 1.3;
}

.source-nav-row:hover:not(.cl-nav-item-active) {
  background: var(--cl-hover);
}

.source-nav-row.cl-nav-item-active {
  font-weight: 500;
}

.source-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.source-dot-all {
  background: var(--cl-text-faint);
}

.source-nav-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-nav-count {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--cl-text-faint);
  font-variant-numeric: tabular-nums;
}

.source-nav-row.cl-nav-item-active .source-nav-count {
  color: var(--cl-text-muted);
}
</style>
