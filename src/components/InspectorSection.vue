<script setup lang="ts">
defineProps<{
  title?: string;
}>();
</script>

<template>
  <section class="inspector-section">
    <h4 v-if="title" class="inspector-section-title">{{ title }}</h4>
    <slot />
  </section>
</template>

<style scoped>
.inspector-section + .inspector-section {
  margin-top: 16px;
  padding-top: 16px;
  border-top: 1px solid var(--cl-border-subtle);
}

.inspector-section-title {
  margin: 0 0 10px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--cl-text-faint);
}
</style>
