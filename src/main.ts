import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";
import { initRipple } from "./lib/ripple";
import { samples, mergeSamples, battery } from "./lib/store";

createApp(App).mount("#app");
initRipple();

// 开发自测钩子（仅 DEV 构建）：在纯浏览器里注入样本，验证曲线/悬浮提示/续航统计
if (import.meta.env.DEV) {
  (window as unknown as Record<string, unknown>).__kbtoolDev = {
    samples,
    mergeSamples,
    battery,
  };
}
