import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  type AppearanceMode,
  getStoredAppearance,
  setAppearance,
} from "../utils/appearance";
import { setAppLocale } from "../i18n";
import type { AppLocale } from "../utils/locale";

const FEEDBACK_URL = "https://github.com/manxisuo/chatilus/issues";

export function useAppearanceMenu() {
  const { t, locale } = useI18n();
  const appearanceMode = ref<AppearanceMode>(getStoredAppearance());
  const privacyDialogVisible = ref(false);

  const appearanceOptions = computed(() => [
    { value: "light" as const, label: t("appearance.light") },
    { value: "dark" as const, label: t("appearance.dark") },
    { value: "system" as const, label: t("appearance.system") },
  ]);

  const languageOptions: Array<{ value: AppLocale; label: string }> = [
    { value: "zh-CN", label: "简体中文" },
    { value: "en", label: "English" },
  ];

  function handleAppearanceCommand(mode: AppearanceMode) {
    appearanceMode.value = mode;
    setAppearance(mode);
  }

  function handleLanguageCommand(next: AppLocale) {
    locale.value = next;
    setAppLocale(next);
  }

  function handleMoreCommand(command: string) {
    if (command === "privacy") {
      privacyDialogVisible.value = true;
      return;
    }
    if (command === "feedback") {
      void openUrl(FEEDBACK_URL);
      return;
    }
    if (command.startsWith("lang:")) {
      handleLanguageCommand(command.slice(5) as AppLocale);
      return;
    }
    if (command.startsWith("appearance:")) {
      handleAppearanceCommand(command.slice(11) as AppearanceMode);
    }
  }

  return {
    appearanceMode,
    appearanceOptions,
    languageOptions,
    privacyDialogVisible,
    handleMoreCommand,
    handleAppearanceCommand,
    handleLanguageCommand,
  };
}
