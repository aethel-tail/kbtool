<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import type { Component } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { battery, samples, mergeSamples, pushSample, loadPrefs } from "./lib/store";
import HomeView from "./views/HomeView.vue";
import KeymapView from "./views/KeymapView.vue";
import LightingView from "./views/LightingView.vue";
import KeyboardView from "./views/KeyboardView.vue";
import SettingsView from "./views/SettingsView.vue";

const pages = [
  { id: "home", label: "主页" },
  { id: "keymap", label: "按键映射" },
  { id: "light", label: "灯光控制" },
  { id: "kbd", label: "键盘设置" },
  { id: "settings", label: "设置" },
];
const active = ref("home");
const views: Record<string, Component> = {
  home: HomeView,
  keymap: KeymapView,
  light: LightingView,
  kbd: KeyboardView,
  settings: SettingsView,
};
const currentView = computed(() => views[active.value]);

interface BatteryPayload {
  percent: number;
  iface: string;
  charging: boolean;
  /** Rust 侧采样时间（unix 秒）；旧后端可能不带，兜底用本地时间 */
  t?: number;
}

const sampleNow = () => Math.floor(Date.now() / 1000);

onMounted(async () => {
  loadPrefs();

  // 先订阅实时事件（期间到达的样本随后会被历史合并按时间戳去重）
  await listen<BatteryPayload>("battery", (e) => {
    const b = e.payload;
    battery.percent = b.percent;
    battery.iface = b.iface;
    battery.charging = b.charging;
    pushSample({ t: Math.floor(b.t ?? sampleNow()), p: b.percent, c: b.charging });
    // 低电量通知由 Rust 轮询线程统一发送（静默托盘模式也无窗口依赖），前端不再重复提醒
  });

  // 持久化历史：重启后全量载入近 35 天；窗口重建时只取上次之后的部分
  try {
    const arr = samples.value;
    const since = arr.length ? arr[arr.length - 1].t : 0;
    const list = await invoke<{ t: number; p: number; c: boolean }[]>("battery_history", {
      since,
      maxDays: 35,
    });
    if (list?.length) {
      mergeSamples(list);
    }
  } catch {
    /* 后台未就绪时忽略 */
  }

  // 初始状态
  try {
    const b = await invoke<BatteryPayload | null>("battery_status");
    if (b) {
      battery.percent = b.percent;
      battery.iface = b.iface;
      battery.charging = b.charging;
      pushSample({ t: Math.floor(b.t ?? sampleNow()), p: b.percent, c: b.charging });
    }
  } catch {
    /* 后台未就绪时忽略 */
  }
});
</script>

<template>
  <div class="app">
    <aside>
      <div class="logo">
        <img class="dot" src="/icon.svg" alt="KBTool" />
        KBTool
      </div>
      <nav>
        <a
          v-for="p in pages"
          :key="p.id"
          :class="{ active: active === p.id }"
          @click="active = p.id"
          >{{ p.label }}</a
        >
      </nav>
      <div class="device-card">
        <div>
          <span class="status-dot" :class="{ off: battery.percent === null }"></span>
          <b>98 键三模键盘</b>
        </div>
        <div class="row">
          <span>连接</span><b>{{ battery.iface }}</b>
        </div>
        <div class="row">
          <span>电量</span><b>{{ battery.percent === null ? "--" : battery.percent + "%" }}</b>
        </div>
        <div class="row">
          <span>状态</span><b>{{ battery.charging ? "充电中" : "电池供电" }}</b>
        </div>
      </div>
    </aside>
    <main>
      <Transition name="page" mode="out-in">
        <component :is="currentView" :key="active" />
      </Transition>
    </main>
  </div>
</template>
