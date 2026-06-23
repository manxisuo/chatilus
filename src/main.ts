import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import App from "./App.vue";
import { initAppearance } from "./utils/appearance";
import "./styles/main.css";

initAppearance();

createApp(App).use(ElementPlus).mount("#app");
