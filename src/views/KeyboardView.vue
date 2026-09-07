<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { kbdPrefs, savePrefs } from "../lib/store";

// 键盘参数（Phase 4 逆向）：休眠 4 档（0=不休眠/1=1分/2=5分/3=30分），响应 5 档（1-5）
const sleepOpts = [
  { v: 0, label: "不休眠" },
  { v: 1, label: "1 分钟" },
  { v: 2, label: "5 分钟" },
  { v: 3, label: "30 分钟" },
];
const respondOpts = [
  { v: 1, label: "快（低延迟）" },
  { v: 2, label: "较快" },
  { v: 3, label: "标准" },
  { v: 4, label: "较慢" },
  { v: 5, label: "慢（防连击）" },
];

const busy = ref(false);
const errMsg = ref("");

async function push() {
  busy.value = true;
  errMsg.value = "";
  try {
    await invoke("set_kbd_params", { sleepMin: kbdPrefs.sleepMin, respondMs: kbdPrefs.respondMs });
    savePrefs();
  } catch (e) {
    errMsg.value = String(e);
  } finally {
    busy.value = false;
  }
}

watch(() => kbdPrefs.sleepMin, push);
watch(() => kbdPrefs.respondMs, push);
</script>

<template>
  <div class="page">
    <h1>键盘设置</h1>
    <div class="sub">性能与省电 · 改动即写入键盘</div>
    <div style="font-size: 12.5px; color: var(--danger)" v-if="errMsg">⚠ {{ errMsg }}</div>
    <div class="card">
      <h3>按键响应时间</h3>
      <div class="remap-panel" style="margin-top: 4px">
        <div
          v-for="o in respondOpts"
          :key="o.v"
          class="opt"
          :class="{ sel: kbdPrefs.respondMs === o.v }"
          @click="kbdPrefs.respondMs = o.v"
        >
          {{ o.label }}
        </div>
      </div>
      <div style="margin-top: 10px; font-size: 12.5px; color: var(--dim)">
        越低延迟越小；若出现双击/连击现象，往慢调。档位含义与官方驱动一致（1-5）。
      </div>
    </div>
    <div class="card">
      <h3>休眠时间</h3>
      <div class="remap-panel" style="margin-top: 4px">
        <div
          v-for="o in sleepOpts"
          :key="o.v"
          class="opt"
          :class="{ sel: kbdPrefs.sleepMin === o.v }"
          @click="kbdPrefs.sleepMin = o.v"
        >
          {{ o.label }}
        </div>
      </div>
      <div style="margin-top: 10px; font-size: 12.5px; color: var(--dim)">
        无操作后进入休眠以省电。休眠中按任意键唤醒，唤醒首键可能丢失（2.4G）。
      </div>
    </div>
  </div>
</template>
