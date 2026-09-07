<script setup lang="ts">
import { ref, onMounted } from "vue";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { invoke } from "@tauri-apps/api/core";
import { settings } from "../lib/store";

const autostart = ref(false);
// 静默启动：默认勾选；仅开机自启开启时可修改（后端持久化，下次开机生效）
const silentStart = ref(true);
const CLOSE_KEY = "kbtool.closeQuits";

onMounted(async () => {
  try {
    autostart.value = await isEnabled();
    settings.autostart = autostart.value;
    // 旧版本已注册的自启项不带 --autostart 参数，重写一次补齐，静默启动才能识别开机拉起
    if (autostart.value) {
      await enable();
    }
  } catch {
    /* 忽略 */
  }
  // 恢复上次的静默启动选择（配置文件缺失即默认开启）
  try {
    silentStart.value = await invoke<boolean>("get_silent_start");
    settings.silentStart = silentStart.value;
  } catch {
    /* 忽略 */
  }
  // 回显后端通知阈值（Rust 线程发通知，重启/重建窗口后以 Rust 侧为准）
  try {
    settings.threshold = await invoke<number>("get_notify_threshold");
  } catch {
    /* 忽略 */
  }
  // 恢复上次的关窗行为选择，并同步给 Rust
  const saved = localStorage.getItem(CLOSE_KEY);
  if (saved !== null) {
    settings.closeQuits = saved === "1";
    try {
      await invoke("set_close_action", { quit: settings.closeQuits });
    } catch {
      /* 忽略 */
    }
  }
});

async function onCloseAction(e: Event) {
  settings.closeQuits = Number((e.target as HTMLSelectElement).value) === 1;
  localStorage.setItem(CLOSE_KEY, settings.closeQuits ? "1" : "0");
  try {
    await invoke("set_close_action", { quit: settings.closeQuits });
  } catch {
    /* 忽略 */
  }
}

async function toggleAutostart() {
  try {
    if (autostart.value) {
      await disable();
      autostart.value = false;
    } else {
      await enable();
      autostart.value = true;
    }
    settings.autostart = autostart.value;
  } catch {
    /* 忽略 */
  }
}

async function onThreshold(e: Event) {
  settings.threshold = Number((e.target as HTMLSelectElement).value);
  // 通知在 Rust 轮询线程发送，阈值需同步过去
  try {
    await invoke("set_notify_threshold", { threshold: settings.threshold });
  } catch {
    /* 忽略 */
  }
}

// 静默启动：仅开机自启开启时可勾选（下次开机自启时直接驻留托盘，不弹主窗口）
async function toggleSilentStart() {
  if (!autostart.value) {
    return;
  }
  silentStart.value = !silentStart.value;
  settings.silentStart = silentStart.value;
  try {
    await invoke("set_silent_start", { silent: silentStart.value });
  } catch {
    /* 忽略 */
  }
}
</script>

<template>
  <div class="page">
    <h1>设置</h1>
    <div class="sub">应用行为</div>
    <div class="card">
      <div class="setting-row">
        <div>
          <div class="t">开机自启</div>
          <div class="d">登录 Windows 后自动在托盘运行</div>
        </div>
        <div class="toggle" :class="{ on: autostart }" @click="toggleAutostart"></div>
      </div>
      <div class="setting-row">
        <div>
          <div class="t">静默启动</div>
          <div class="d">
            开机自启时直接驻留后台，不弹出主窗口（需先勾选上方“开机自启”才能修改）
          </div>
        </div>
        <div
          class="toggle"
          :class="{ on: silentStart, disabled: !autostart }"
          :title="autostart ? '' : '需先开启开机自启'"
          @click="toggleSilentStart"
        ></div>
      </div>
      <div class="setting-row">
        <div>
          <div class="t">低电量通知</div>
          <div class="d">低于阈值时发送系统通知</div>
        </div>
        <select :value="settings.threshold" @change="onThreshold">
          <option :value="30">30%</option>
          <option :value="20">20%</option>
          <option :value="10">10%</option>
        </select>
      </div>
      <div class="setting-row">
        <div>
          <div class="t">轮询间隔</div>
          <div class="d">电量查询频率（更长更省电）</div>
        </div>
        <select v-model.number="settings.pollSec">
          <option :value="15">15 秒</option>
          <option :value="30">30 秒</option>
          <option :value="60">1 分钟</option>
          <option :value="300">5 分钟</option>
        </select>
      </div>
      <div class="setting-row">
        <div>
          <div class="t">关闭窗口时</div>
          <div class="d">
            点 X 或任务栏关闭后，收起至托盘常驻（省内存）；选"直接退出"则立即结束程序
          </div>
        </div>
        <select :value="settings.closeQuits ? 1 : 0" @change="onCloseAction">
          <option :value="0">收起至托盘（省内存）</option>
          <option :value="1">直接退出</option>
        </select>
      </div>
    </div>
    <div class="card">
      <div class="setting-row">
        <div>
          <div class="t">关于</div>
          <div class="d">KBTool v1.1.0 · MIT 开源 · 非官方工具</div>
        </div>
        <span style="font-size: 12.5px; color: var(--dimmer)">docs/操作指南.md</span>
      </div>
    </div>
    <div class="hint">
      ⚠ 蓝牙模式下固件不开放配置通道，无法读取电量或修改设置。请切换 2.4G 或 USB 有线连接。
    </div>
    <div class="footer-note">本工具通过运行时流量观察实现兼容，与任何键盘厂商无关。</div>
  </div>
</template>
