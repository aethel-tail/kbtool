import { reactive, shallowRef } from "vue";

export interface BatteryState {
  percent: number | null;
  iface: string;
  charging: boolean;
}

export const battery = reactive<BatteryState>({ percent: null, iface: "未连接", charging: false });

export const settings = reactive({
  threshold: 30,
  pollSec: 15,
  autostart: false,
  // 静默启动：默认开启；仅开机自启开启时可勾选（由 Rust 持久化到 app_config/silent_start.json）
  silentStart: true,
  // 关窗行为：false = 收起（销毁窗口、托盘常驻）；true = 直接退出程序
  closeQuits: false,
});

// ===== 电量历史（持久化曲线：近一周展示 / 近一月统计）=====
// Rust 轮询线程每 15s 把采样追加到 app_data/battery_history.log（关窗托盘常驻、重启均不丢）；
// 前端维护一份内存镜像供画图与统计，启动时经 battery_history 命令增量合并（按 t 单调去重）。
// 样本量大（≈15s 一条），用 shallowRef 整体替换而非深响应数组，避免逐对象代理开销。
export interface Sample {
  /** unix 秒 */
  t: number;
  /** 电量百分比 */
  p: number;
  /** 是否充电中 */
  c: boolean;
}

/** 内存保留窗口（Rust 文件侧按同值裁剪，保持一致） */
export const SAMPLE_KEEP_SEC = 35 * 86400;

export const samples = shallowRef<Sample[]>([]);

function trimSamples() {
  const arr = samples.value;
  if (!arr.length) {
    return;
  }
  const cutoff = Date.now() / 1000 - SAMPLE_KEEP_SEC;
  let i = 0;
  while (i < arr.length && arr[i].t < cutoff) {
    i++;
  }
  if (i) {
    samples.value = arr.slice(i);
  }
}

/** 追加一条实时采样（与历史加载竞态下按时间戳去重） */
export function pushSample(s: Sample) {
  const arr = samples.value;
  if (arr.length && s.t <= arr[arr.length - 1].t) {
    return;
  }
  samples.value = [...arr, s];
  trimSamples();
}

/** 批量合并历史（命令已过滤 since，这里再防御性去重） */
export function mergeSamples(list: Sample[]) {
  if (!list.length) {
    return;
  }
  const arr = samples.value;
  const last = arr.length ? arr[arr.length - 1].t : 0;
  const fresh: Sample[] = [];
  for (const s of list) {
    if (s.t > last) {
      fresh.push(s);
    }
  }
  if (fresh.length) {
    samples.value = arr.concat(fresh);
    trimSamples();
  }
}

// 低电量通知去重已下沉 Rust（轮询线程，静默托盘模式也生效），见 src-tauri/src/lib.rs

// ===== 灯光/键盘参数偏好（跨页面切换 + 跨重启持久化）=====
// 语义见 findings.md：模式 1-19(0=关灯) RGB 多彩 亮度1-5 速度1-5 方向0-3；休眠档0-3 响应档1-5
export const lightPrefs = reactive({
  mode: 1,
  lightsOff: false,
  colorful: true,
  color: "#ff0000",
  brightness: 5,
  speed: 3,
  direction: 0,
});
export const kbdPrefs = reactive({
  sleepMin: 2, // 0=不休眠 1=1分钟 2=5分钟 3=30分钟
  respondMs: 2, // 1-5 档
});

const LS_LIGHT = "kbtool.lightPrefs";
const LS_KBD = "kbtool.kbdPrefs";

export function loadPrefs() {
  try {
    const l = JSON.parse(localStorage.getItem(LS_LIGHT) || "null");
    if (l && typeof l === "object") {
      Object.assign(lightPrefs, l);
    }
    const k = JSON.parse(localStorage.getItem(LS_KBD) || "null");
    if (k && typeof k === "object") {
      Object.assign(kbdPrefs, k);
    }
  } catch {
    /* 损坏的存储忽略 */
  }
}

export function savePrefs() {
  try {
    localStorage.setItem(LS_LIGHT, JSON.stringify(lightPrefs));
    localStorage.setItem(LS_KBD, JSON.stringify(kbdPrefs));
  } catch {
    /* 忽略 */
  }
}
