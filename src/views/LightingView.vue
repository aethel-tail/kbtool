<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { lightPrefs, savePrefs } from "../lib/store";

// 模式号与驱动 UI 顺序一致（Phase 4 实测第 N 项 = 模式号 N；0 = 关闭背光）
// 能力矩阵来自用户实测：静态恒亮无速度；百花争艳/光谱循环固定多彩（禁颜色）；
// 方向：多彩纵横=上下，随波逐流/峰回路转/川流不息/斜风细雨=左右
interface ModeDef {
  name: string;
  noSpeed?: boolean;
  noColor?: boolean;
  dir?: "lr" | "ud";
}
const modes: ModeDef[] = [
  { name: "静态恒亮", noSpeed: true },
  { name: "单熄灭" }, // 模式号 2（实测单点亮/单熄灭位置对调）
  { name: "单点亮" }, // 模式号 3
  { name: "繁星点点" },
  { name: "漫天飞雪" },
  { name: "百花争艳", noColor: true },
  { name: "动态呼吸" },
  { name: "光谱循环", noColor: true },
  { name: "彩泉涌动" },
  { name: "多彩纵横", dir: "ud" },
  { name: "随波逐流", dir: "lr" },
  { name: "峰回路转", dir: "lr" },
  { name: "一触即发" },
  { name: "一石二鸟" },
  { name: "涟漪扩散" },
  { name: "川流不息", dir: "lr" },
  { name: "重峦叠嶂" },
  { name: "斜风细雨", dir: "lr" },
  { name: "来回穿梭" },
];
const palette = [
  "#ffffff",
  "#ff0000",
  "#ff8000",
  "#ffff00",
  "#00ff00",
  "#00ffff",
  "#0000ff",
  "#ff00ff",
  "#000000",
];
const PAL_LABELS = ["白", "红", "橙", "黄", "绿", "青", "蓝", "粉", "黑"];

const busy = ref(false);
const errMsg = ref("");

const cur = computed(() => modes[lightPrefs.mode - 1]);
const canColor = computed(() => !cur.value.noColor);
const canSpeed = computed(() => !cur.value.noSpeed);
// 任何交互锁定时统一禁用（pointer-events 父级 none 会被子元素 auto 覆盖，故每个可点元素都需判断）
const lockAll = computed(() => lightPrefs.lightsOff);
const lockColor = computed(() => lightPrefs.lightsOff || !canColor.value);

function dirOpts(m: ModeDef) {
  if (m.dir === "ud") {
    return [
      { v: 2, label: "上→下" },
      { v: 3, label: "下→上" },
    ];
  }
  if (m.dir === "lr") {
    return [
      { v: 0, label: "左→右" },
      { v: 1, label: "右→左" },
    ];
  }
  return [];
}
const dirs = computed(() => dirOpts(cur.value));

function hex2rgb(h: string): [number, number, number] {
  const v = parseInt(h.slice(1), 16);
  return [(v >> 16) & 255, (v >> 8) & 255, v & 255];
}

async function push() {
  busy.value = true;
  errMsg.value = "";
  try {
    const [r, g, b] = hex2rgb(lightPrefs.color);
    // 强制多彩的模式固定 colorful=true
    await invoke("set_light", {
      mode: lightPrefs.lightsOff ? 0 : lightPrefs.mode,
      r,
      g,
      b,
      colorful: canColor.value ? lightPrefs.colorful : true,
      brightness: lightPrefs.brightness,
      speed: lightPrefs.speed,
      direction: lightPrefs.direction,
    });
    savePrefs();
  } catch (e) {
    errMsg.value = String(e);
  } finally {
    busy.value = false;
  }
}

function pickMode(i: number) {
  lightPrefs.mode = i + 1;
  lightPrefs.lightsOff = false;
  // 方向组切换时校正编码域；不可调色/速度的模式复位相关状态
  const d = dirOpts(modes[i]);
  if (d.length && !d.some((x) => x.v === lightPrefs.direction)) {
    lightPrefs.direction = d[0].v;
  }
  if (modes[i].noColor) {
    lightPrefs.colorful = true;
  }
  push();
}

function off() {
  lightPrefs.lightsOff = true;
  savePrefs();
  push();
}

// 参数变更即发（pickMode/off 已显式 push，这里只覆盖滑杆/色板/开关类直接改 store 的路径）
watch(
  () => lightPrefs.colorful,
  () => {
    if (canColor.value && !lockAll.value) {
      push();
    }
  },
);
watch(
  () => lightPrefs.brightness,
  () => {
    if (!lockAll.value) {
      push();
    }
  },
);
watch(
  () => lightPrefs.speed,
  () => {
    if (canSpeed.value && !lockAll.value) {
      push();
    }
  },
);
watch(
  () => lightPrefs.direction,
  () => push(),
);
watch(
  () => lightPrefs.color,
  () => {
    if (canColor.value && !lockAll.value) {
      push();
    }
  },
);
</script>

<template>
  <div class="page">
    <h1>灯光控制</h1>
    <div class="sub">点击即生效 · 无光污染，从关灯开始</div>
    <div style="font-size: 12.5px; color: var(--danger)" v-if="errMsg">⚠ {{ errMsg }}</div>

    <div class="card" style="padding: 14px 18px; border-color: #3a4152">
      <div class="light-toggle-row" style="padding: 0">
        <div style="flex: 1">
          <div class="big">键盘灯光 {{ lightPrefs.lightsOff ? "· 已关闭" : "" }}</div>
          <div class="desc">点击模式即开灯；关灯后所有灯效停止（指示灯除外）</div>
        </div>
        <button class="btn-danger" :disabled="busy" @click="off">关灯</button>
      </div>
    </div>

    <!-- 两列：模式（宽） | 颜色/参数（窄），保证 860 高内无滚动条 -->
    <div style="display: grid; grid-template-columns: 1.25fr 1fr; gap: 16px; align-items: start">
      <div class="card" style="padding: 16px; margin-bottom: 0">
        <h3>
          灯效模式
          <span style="font-weight: 400; color: var(--dimmer)"
            >· {{ lightPrefs.lightsOff ? "已关闭" : cur.name }}</span
          >
        </h3>
        <div
          class="mode-grid"
          style="grid-template-columns: repeat(5, 1fr); gap: 6px; margin-top: 10px"
        >
          <div
            v-for="(m, i) in modes"
            :key="m.name"
            class="mode"
            :class="{ sel: !lightPrefs.lightsOff && lightPrefs.mode === i + 1 }"
            @click="pickMode(i)"
          >
            {{ m.name }}
          </div>
        </div>
      </div>

      <div>
        <div class="card" style="padding: 16px; margin-bottom: 16px">
          <h3>颜色与多彩</h3>
          <div class="remap-panel" style="gap: 6px; margin-top: 10px">
            <div
              v-for="(c, i) in palette"
              :key="c"
              class="sw-dot"
              :class="{ sel: canColor && !lightPrefs.colorful && lightPrefs.color === c }"
              :style="{
                background: c,
                opacity: lockColor ? 0.35 : 1,
                pointerEvents: lockColor ? 'none' : 'auto',
              }"
              :title="PAL_LABELS[i]"
              @click="
                lightPrefs.colorful = false;
                lightPrefs.color = c;
              "
            ></div>
            <input
              type="color"
              :value="lightPrefs.color"
              :disabled="lockColor"
              @input="
                lightPrefs.colorful = false;
                lightPrefs.color = ($event.target as HTMLInputElement).value;
              "
              class="sw-dot"
              style="
                padding: 0;
                cursor: pointer;
                background: conic-gradient(#ef4444, #f59e0b, #4ade80, #3b82f6, #a855f7, #ef4444);
                border: none;
              "
              :title="'自定义'"
            />
          </div>
          <div class="light-toggle-row" style="padding: 0; margin-top: 10px">
            <div style="flex: 1">
              <div class="big" style="font-size: 14px">多彩效果</div>
              <div class="desc">{{ canColor ? "开 = 彩色循环" : "此模式固定多彩" }}</div>
            </div>
            <div
              class="toggle"
              :class="{ on: lightPrefs.colorful }"
              :style="{ opacity: lockColor ? 0.35 : 1 }"
              @click="!lockColor && (lightPrefs.colorful = !lightPrefs.colorful)"
            ></div>
          </div>
        </div>

        <div class="card" style="padding: 16px; margin-bottom: 0">
          <h3>参数</h3>
          <div class="slider-row" style="margin-bottom: 10px">
            <div class="label">亮度</div>
            <input
              type="range"
              class="slider"
              min="1"
              max="5"
              v-model.number="lightPrefs.brightness"
              :disabled="lockAll"
            />
            <div class="val">{{ lightPrefs.brightness }} / 5</div>
          </div>
          <div class="slider-row" style="margin-bottom: 10px">
            <div class="label">速度</div>
            <template v-if="canSpeed">
              <input
                type="range"
                class="slider"
                min="1"
                max="5"
                v-model.number="lightPrefs.speed"
                :disabled="lockAll"
              />
              <div class="val">{{ lightPrefs.speed }} / 5</div>
            </template>
            <div
              v-else
              class="val"
              style="width: auto; font-weight: 400; color: var(--dimmer); text-align: left"
            >
              此模式不支持
            </div>
          </div>
          <div class="slider-row" v-if="dirs.length" style="margin-bottom: 0">
            <div class="label">方向</div>
            <div class="remap-panel" style="gap: 6px">
              <div
                v-for="d in dirs"
                :key="d.v"
                class="opt"
                style="padding: 6px 12px; font-size: 12.5px"
                :class="{ sel: lightPrefs.direction === d.v }"
                :style="{ pointerEvents: lockAll ? 'none' : 'auto', opacity: lockAll ? 0.5 : 1 }"
                @click="lightPrefs.direction = d.v"
              >
                {{ d.label }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
