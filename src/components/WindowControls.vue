<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";

const appWindow = getCurrentWindow();

async function minimize() {
  await appWindow.minimize();
}

async function toggleMaximize() {
  await appWindow.toggleMaximize();
}

async function close() {
  await appWindow.close();
}
</script>

<template>
  <div class="window-controls" role="group" :aria-label="'Window'">
    <button type="button" class="win-btn" title="Minimize" @click="minimize">−</button>
    <button type="button" class="win-btn" title="Maximize" @click="toggleMaximize">□</button>
    <button type="button" class="win-btn win-btn-close" title="Close" @click="close">×</button>
  </div>
</template>

<style scoped>
.window-controls {
  display: flex;
  align-items: stretch;
  flex-shrink: 0;
  margin-left: 4px;
  border-left: 1px solid var(--cl-border-subtle);
}

.win-btn {
  border: none;
  background: transparent;
  width: 40px;
  height: var(--cl-toolbar-height);
  font-size: 14px;
  line-height: 1;
  color: var(--cl-text-muted);
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}

.win-btn:hover {
  background: var(--cl-hover);
  color: var(--cl-text);
}

.win-btn-close:hover {
  background: #e81123;
  color: #fff;
}
</style>
