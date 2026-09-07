import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// vite 8 对 localhost 默认只绑 IPv6 回环 [::1]，而 tauri devUrl 走 127.0.0.1 (IPv4)，
// 故本地开发显式绑 127.0.0.1；TAURI_DEV_HOST（远程设备调试）时交由环境变量。
// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
