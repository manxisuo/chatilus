<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";

export interface GalleryImage {
  path: string;
  fileKey: string;
}

const props = defineProps<{
  visible: boolean;
  images: GalleryImage[];
  initialIndex: number;
  resolveSrc: (path: string) => string;
  onImageError?: (path: string) => void;
  captions?: string[];
}>();

const emit = defineEmits<{
  "update:visible": [value: boolean];
}>();

const currentIndex = ref(0);

const currentImage = computed(() => props.images[currentIndex.value] ?? null);

const captionText = computed(() => {
  if (!props.captions?.length) return "";
  return props.captions[currentIndex.value] ?? "";
});

const counterText = computed(() => {
  if (props.images.length === 0) return "";
  return `${currentIndex.value + 1} / ${props.images.length}`;
});

const canGoPrev = computed(() => currentIndex.value > 0);
const canGoNext = computed(
  () => currentIndex.value < props.images.length - 1,
);

watch(
  () => props.visible,
  (open) => {
    if (!open) return;
    currentIndex.value = Math.min(
      Math.max(props.initialIndex, 0),
      Math.max(props.images.length - 1, 0),
    );
  },
);

watch(
  () => props.initialIndex,
  (index) => {
    if (props.visible) {
      currentIndex.value = index;
    }
  },
);

function close() {
  emit("update:visible", false);
}

function goPrev() {
  if (canGoPrev.value) {
    currentIndex.value -= 1;
  }
}

function goNext() {
  if (canGoNext.value) {
    currentIndex.value += 1;
  }
}

function onKeydown(event: KeyboardEvent) {
  if (!props.visible) return;

  if (event.key === "Escape") {
    close();
  } else if (event.key === "ArrowLeft") {
    goPrev();
  } else if (event.key === "ArrowRight") {
    goNext();
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible && currentImage"
      class="lightbox"
      @click.self="close"
    >
      <button class="close-btn" type="button" aria-label="关闭" @click="close">
        ✕
      </button>

      <button
        class="nav-btn prev"
        type="button"
        aria-label="上一张"
        :disabled="!canGoPrev"
        @click.stop="goPrev"
      >
        ‹
      </button>

      <div class="stage" @click.stop>
        <img
          :src="resolveSrc(currentImage.path)"
          :alt="currentImage.fileKey"
          class="preview"
          @error="onImageError?.(currentImage.path)"
        />
        <div class="meta">
          <span v-if="captionText" class="caption">{{ captionText }}</span>
          <span>{{ counterText }}</span>
        </div>
      </div>

      <button
        class="nav-btn next"
        type="button"
        aria-label="下一张"
        :disabled="!canGoNext"
        @click.stop="goNext"
      >
        ›
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.lightbox {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 24px;
  background: rgba(0, 0, 0, 0.88);
}

.stage {
  display: flex;
  flex-direction: column;
  align-items: center;
  max-width: calc(100vw - 160px);
  max-height: calc(100vh - 48px);
}

.preview {
  max-width: 100%;
  max-height: calc(100vh - 96px);
  object-fit: contain;
  border-radius: 8px;
  background: #111;
}

.meta {
  margin-top: 12px;
  color: rgba(255, 255, 255, 0.85);
  font-size: 14px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  align-items: center;
}

.caption {
  font-size: 13px;
  opacity: 0.9;
  max-width: 70vw;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.close-btn {
  position: absolute;
  top: 16px;
  right: 20px;
  border: none;
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  width: 36px;
  height: 36px;
  border-radius: 999px;
  cursor: pointer;
  font-size: 18px;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.nav-btn {
  flex-shrink: 0;
  width: 44px;
  height: 44px;
  border: none;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  font-size: 28px;
  line-height: 1;
  cursor: pointer;
}

.nav-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.22);
}

.nav-btn:disabled {
  opacity: 0.25;
  cursor: not-allowed;
}
</style>
