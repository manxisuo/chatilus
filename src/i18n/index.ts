import { createI18n } from "vue-i18n";
import { getStoredLocale, setStoredLocale } from "../utils/locale";
import en from "./locales/en";
import zhCN from "./locales/zh-CN";

const locale = getStoredLocale();
setStoredLocale(locale);

export const i18n = createI18n({
  legacy: false,
  locale,
  fallbackLocale: "zh-CN",
  messages: {
    "zh-CN": zhCN,
    en,
  },
});

export function setAppLocale(locale: "zh-CN" | "en") {
  i18n.global.locale.value = locale;
  setStoredLocale(locale);
}
