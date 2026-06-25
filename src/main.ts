import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import App from "./App.vue";
import { i18n } from "./i18n";
import { initAppearance } from "./utils/appearance";
import "./styles/main.css";

initAppearance();

createApp(App).use(i18n).use(ElementPlus).mount("#app");
