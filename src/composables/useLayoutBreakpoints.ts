import { computed, onMounted, onUnmounted, ref } from "vue";
import { LAYOUT_BREAKPOINTS } from "../utils/layout";

export function useLayoutBreakpoints() {
  const width = ref(
    typeof window !== "undefined" ? window.innerWidth : LAYOUT_BREAKPOINTS.hideRightPanel + 1,
  );

  function update() {
    width.value = window.innerWidth;
  }

  onMounted(() => {
    update();
    window.addEventListener("resize", update);
  });

  onUnmounted(() => {
    window.removeEventListener("resize", update);
  });

  const showRightPanel = computed(
    () => width.value >= LAYOUT_BREAKPOINTS.hideRightPanel,
  );

  const compactLeftPanel = computed(
    () =>
      width.value < LAYOUT_BREAKPOINTS.compactLeftPanel &&
      width.value >= LAYOUT_BREAKPOINTS.singleColumn,
  );

  return {
    width,
    showRightPanel,
    compactLeftPanel,
  };
}
