<script setup lang="ts">
import { onUnmounted, ref, watch } from "vue";

const props = defineProps<{
  path: string;
  alt: string;
  scrollRoot: HTMLElement | null;
  loadFailed: (path: string) => boolean;
  resolveSrc: (path: string) => string;
  onImageError: (path: string) => void;
  missingLabel: string;
}>();

const host = ref<HTMLElement | null>(null);
const visible = ref(false);
let observer: IntersectionObserver | null = null;

function disconnectObserver() {
  observer?.disconnect();
  observer = null;
}

function attachObserver(root: HTMLElement | null) {
  disconnectObserver();
  if (!host.value || visible.value) return;

  if (!root) {
    visible.value = true;
    return;
  }

  observer = new IntersectionObserver(
    ([entry]) => {
      if (!entry?.isIntersecting) return;
      visible.value = true;
      disconnectObserver();
    },
    { root, rootMargin: "240px 0px", threshold: 0 },
  );
  observer.observe(host.value);
}

watch(
  () => props.scrollRoot,
  (root) => attachObserver(root),
  { flush: "post" },
);

watch(host, (el) => {
  if (el) attachObserver(props.scrollRoot);
});

onUnmounted(() => {
  disconnectObserver();
});
</script>

<template>
  <div ref="host" class="thumb-media">
    <img
      v-if="visible && path && !loadFailed(path)"
      :src="resolveSrc(path)"
      :alt="alt"
      decoding="async"
      @error="onImageError(path)"
    />
    <div v-else-if="visible && loadFailed(path)" class="thumb-missing">
      {{ missingLabel }}
    </div>
  </div>
</template>

<style scoped>
.thumb-media {
  width: 100%;
  height: 100%;
}

.thumb-media img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
  background: var(--cl-bg);
}

.thumb-missing {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--cl-text-muted);
  background: var(--cl-selected);
}
</style>
