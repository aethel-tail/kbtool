<script setup lang="ts">
import { ref, reactive, computed, onMounted, onBeforeUnmount, watch } from "vue";
import { battery, samples } from "../lib/store";
import type { Sample } from "../lib/store";

const R = 66;
const CIRC = 2 * Math.PI * R;

const ringColor = computed(() => {
  const p = battery.percent;
  if (p === null) {
    return "#5c6472";
  }
  if (p > 50) {
    return "#4ade80";
  }
  if (p > 30) {
    return "#facc15";
  }
  return "#ef4444";
});
const dashOffset = computed(() => CIRC * (1 - (battery.percent ?? 0) / 100));
const pctText = computed(() => (battery.percent === null ? "--" : String(battery.percent)));
const ifaceText = computed(() => battery.iface);
const chargeBadge = computed(() => (battery.charging ? "⚡ 充电中" : "● 未充电 · 使用中"));

// =========================================================
// 电量曲线（近一周）：canvas + hover 十字线 + HTML 气泡
// =========================================================
const WEEK_SEC = 7 * 86400;
const PAD = { l: 42, r: 12, t: 12, b: 26 }; // 左留 Y 轴标签，下留日期标签
const nowSec = () => Math.floor(Date.now() / 1000);

interface Geo {
  w: number;
  h: number;
  pw: number;
  ph: number;
  t0: number;
  t1: number;
}
const chart = ref<HTMLCanvasElement | null>(null);

function getGeo(): Geo | null {
  const cv = chart.value;
  if (!cv) {
    return null;
  }
  const w = cv.clientWidth,
    h = cv.clientHeight;
  if (!w || !h) {
    return null;
  }
  const t1 = nowSec();
  return { w, h, pw: w - PAD.l - PAD.r, ph: h - PAD.t - PAD.b, t0: t1 - WEEK_SEC, t1 };
}
/** 时间+电量 -> 画布 CSS 像素坐标 */
function px(g: Geo, t: number, p: number) {
  return {
    x: PAD.l + ((t - g.t0) / WEEK_SEC) * g.pw,
    y: PAD.t + (1 - p / 100) * g.ph,
  };
}

/** 悬停状态：draw() 用它画十字线；气泡是独立 DOM */
const hover = reactive<{ t: number | null; p: number; c: boolean }>({ t: null, p: 0, c: false });
const tip = reactive({ show: false, p: 0, c: false, time: "", left: 0, top: 0, above: true });

const FONT = "10px 'Segoe UI','Microsoft YaHei UI','Microsoft YaHei',sans-serif";
const WDAYS = ["日", "一", "二", "三", "四", "五", "六"];

function fmtClock(ts: number) {
  const d = new Date(ts * 1000);
  const p2 = (n: number) => String(n).padStart(2, "0");
  return `${d.getMonth() + 1}/${d.getDate()} ${p2(d.getHours())}:${p2(d.getMinutes())}`;
}

/** 取时间戳 >= t 的首个样本下标（数组按 t 升序） */
function firstGe(arr: Sample[], t: number) {
  let lo = 0,
    hi = arr.length;
  while (lo < hi) {
    const m = (lo + hi) >> 1;
    if (arr[m].t < t) {
      lo = m + 1;
    } else {
      hi = m;
    }
  }
  return lo;
}

function draw() {
  const cv = chart.value;
  const g = getGeo();
  if (!cv || !g) {
    return;
  }
  const dpr = window.devicePixelRatio || 1;
  const bw = Math.round(g.w * dpr),
    bh = Math.round(g.h * dpr);
  if (cv.width !== bw || cv.height !== bh) {
    cv.width = bw;
    cv.height = bh;
  }
  const ctx = cv.getContext("2d");
  if (!ctx) {
    return;
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, g.w, g.h);

  const top = PAD.t,
    bottom = PAD.t + g.ph;

  // ---- 横向网格：电量档位 ----
  ctx.lineWidth = 1;
  for (let i = 0; i <= 4; i++) {
    const p = 100 - i * 25;
    const y = PAD.t + (g.ph * i) / 4;
    ctx.strokeStyle = "#222834";
    ctx.beginPath();
    ctx.moveTo(PAD.l, y);
    ctx.lineTo(PAD.l + g.pw, y);
    ctx.stroke();
    ctx.fillStyle = "#5c6472";
    ctx.font = FONT;
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";
    ctx.fillText(`${p}%`, PAD.l - 6, y);
  }
  // 30% 参考线
  const y30 = PAD.t + (1 - 0.3) * g.ph;
  ctx.strokeStyle = "#7f1d1d";
  ctx.setLineDash([4, 4]);
  ctx.beginPath();
  ctx.moveTo(PAD.l, y30);
  ctx.lineTo(PAD.l + g.pw, y30);
  ctx.stroke();
  ctx.setLineDash([]);

  // ---- 竖向网格：每天 0 点 + 日期标签 ----
  const midnight = new Date();
  midnight.setHours(0, 0, 0, 0);
  ctx.textAlign = "center";
  for (let d = 8; d >= 0; d--) {
    const mt = Math.floor(midnight.getTime() / 1000) - d * 86400;
    if (mt <= g.t0 || mt > g.t1) {
      continue;
    }
    const x = PAD.l + ((mt - g.t0) / WEEK_SEC) * g.pw;
    ctx.strokeStyle = "#1d222d";
    ctx.beginPath();
    ctx.moveTo(x, top);
    ctx.lineTo(x, bottom);
    ctx.stroke();
    const dd = new Date(mt * 1000);
    ctx.fillStyle = "#5c6472";
    ctx.textBaseline = "alphabetic";
    ctx.fillText(`${dd.getMonth() + 1}/${dd.getDate()}`, x, g.h - 18);
    ctx.fillText(`周${WDAYS[dd.getDay()]}`, x, g.h - 7);
  }

  // ---- 窗口内样本 ----
  const arr = samples.value;
  const vis: Sample[] = [];
  if (arr.length) {
    for (let i = firstGe(arr, g.t0); i < arr.length && arr[i].t <= g.t1; i++) {
      vis.push(arr[i]);
    }
  }
  if (vis.length === 0) {
    const msg = arr.length
      ? "近一周无采样（设备可能长时间离线）"
      : battery.percent === null
        ? "设备未连接 —— 在线后自动记录电量历史"
        : "正在采集数据…";
    ctx.fillStyle = "#5c6472";
    ctx.font = "12px 'Segoe UI','Microsoft YaHei UI','Microsoft YaHei',sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(msg, PAD.l + g.pw / 2, top + g.ph / 2);
    hover.t = null;
    tip.show = false;
    return;
  }

  // ---- 面积填充 ----
  const grad = ctx.createLinearGradient(0, top, 0, bottom);
  grad.addColorStop(0, "rgba(74,222,128,.18)");
  grad.addColorStop(1, "rgba(74,222,128,0)");
  ctx.beginPath();
  vis.forEach((s, i) => {
    const { x, y } = px(g, s.t, s.p);
    if (i) {
      ctx.lineTo(x, y);
    } else {
      ctx.moveTo(x, y);
    }
  });
  ctx.lineTo(px(g, vis[vis.length - 1].t, 0).x, bottom);
  ctx.lineTo(px(g, vis[0].t, 0).x, bottom);
  ctx.closePath();
  ctx.fillStyle = grad;
  ctx.fill();

  // ---- 折线：按充电状态分段着色（绿=使用，黄=充电）----
  const stroke = (from: number, to: number) => {
    if (to - from < 1) {
      return;
    }
    const c = vis[from].c;
    ctx.beginPath();
    for (let i = from; i < to; i++) {
      const { x, y } = px(g, vis[i].t, vis[i].p);
      if (i === from) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }
    ctx.strokeStyle = c ? "#facc15" : "#4ade80";
    ctx.lineWidth = 2;
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    ctx.stroke();
  };
  let seg = 0;
  for (let i = 1; i < vis.length; i++) {
    if (vis[i].c !== vis[i - 1].c) {
      stroke(seg, i);
      seg = i;
    }
  }
  stroke(seg, vis.length);

  // ---- hover 十字线 + 圆点 ----
  if (hover.t !== null) {
    const { x, y } = px(g, hover.t, hover.p);
    ctx.strokeStyle = "rgba(232,234,240,.45)";
    ctx.setLineDash([4, 3]);
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(x, top);
    ctx.lineTo(x, bottom);
    ctx.stroke();
    ctx.setLineDash([]);
    const dot = hover.c ? "#facc15" : "#4ade80";
    ctx.globalAlpha = 0.22;
    ctx.fillStyle = dot;
    ctx.beginPath();
    ctx.arc(x, y, 8, 0, Math.PI * 2);
    ctx.fill();
    ctx.globalAlpha = 1;
    ctx.beginPath();
    ctx.arc(x, y, 3.5, 0, Math.PI * 2);
    ctx.fill();
  }
}

let raf = 0,
  tmo = 0;
function scheduleDraw() {
  if (raf || tmo) {
    return;
  }
  // 优先 rAF（与合成器同步）；被遮挡/最小化/无头环境下 rAF 可能不触发，50ms 兑底
  tmo = window.setTimeout(() => {
    tmo = 0;
    if (raf) {
      window.cancelAnimationFrame(raf);
      raf = 0;
    }
    draw();
  }, 50);
  raf = requestAnimationFrame(() => {
    raf = 0;
    if (tmo) {
      window.clearTimeout(tmo);
      tmo = 0;
    }
    draw();
  });
}

function onMove(e: MouseEvent) {
  const cv = chart.value,
    g = getGeo();
  if (!cv || !g) {
    return;
  }
  const rect = cv.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;
  const inPlot = mx >= PAD.l && mx <= PAD.l + g.pw && my >= PAD.t && my <= PAD.t + g.ph;
  const arr = samples.value;
  let best: Sample | null = null;
  if (inPlot && arr.length) {
    const tq = g.t0 + ((mx - PAD.l) / g.pw) * WEEK_SEC;
    let i = firstGe(arr, tq);
    // 取左右最近样本
    if (i > 0 && (i === arr.length || tq - arr[i - 1].t <= arr[i].t - tq)) {
      i--;
    }
    if (i < arr.length) {
      best = arr[i];
    }
    // 超出窗口或离线空档太大（>6h）时不吸附，避免误导
    if (best && (best.t < g.t0 || best.t > g.t1 || Math.abs(best.t - tq) > 6 * 3600)) {
      best = null;
    }
  }
  hover.t = best ? best.t : null;
  hover.p = best ? best.p : 0;
  hover.c = best ? best.c : false;
  tip.show = !!best;
  if (best) {
    tip.p = best.p;
    tip.c = best.c;
    tip.time = fmtClock(best.t);
    const { x, y } = px(g, best.t, best.p);
    tip.above = y - 10 > 66;
    tip.left = Math.min(Math.max(x + 14, 8), g.w - 178);
    tip.top = y - 12;
    if (!tip.above) {
      tip.left = Math.min(Math.max(x - 60, 8), g.w - 178);
      tip.top = y + 16;
    }
  }
  scheduleDraw();
}
function onLeave() {
  hover.t = null;
  tip.show = false;
  scheduleDraw();
}

// =========================================================
// 近一月统计：平均续航 / 预计续航 / 近一周最低
// =========================================================
const MONTH_SEC = 30 * 86400;

const meta = reactive({
  /** 平均满电续航（小时），由近 30 天放电段推算 */
  hpd: null as number | null,
  /** 纳入统计的放电段数 */
  segs: 0,
  /** 近一周最低电量（窗口内） */
  weekLow: null as number | null,
});

/** 扫描近 30 天放电段：
 *  按“非充电”连续段切分；只累计 <15% 的平稳下降（排除拔线“假满电”83→45 这类骤降），
 *  段耗电 ≥10% 且持续 ≥0.5h 才算有效观测；跨出 30 天边界的段不统计（避免残缺）。
 *  续航(小时/100%) = 段时长 × 100 / 段耗电，最后按总耗电加权汇总。 */
function updateStats() {
  const arr = samples.value;
  const now = nowSec();
  const monthStart = now - MONTH_SEC;
  let run: { st: number; drop: number; prev: number; last: number } | null = null;
  let totDurH = 0,
    totDrop = 0,
    segs = 0;
  const finish = () => {
    if (!run) {
      return;
    }
    if (run.st >= monthStart) {
      const h = (run.last - run.st) / 3600;
      if (run.drop >= 10 && h >= 0.5) {
        segs++;
        totDrop += run.drop;
        totDurH += h;
      }
    }
    run = null;
  };
  for (const s of arr) {
    if (!s.c) {
      if (!run) {
        run = { st: s.t, drop: 0, prev: s.p, last: s.t };
      } else {
        const d = run.prev - s.p; // 平稳放电步进（0,15]
        if (d > 0 && d <= 15) {
          run.drop += d;
        }
        run.prev = s.p;
        run.last = s.t;
      }
    } else {
      finish();
    }
  }
  finish(); // 进行中的段也算（误差有限，且让“刚用几天”就有数据）
  meta.segs = segs;
  meta.hpd = totDrop >= 10 ? (totDurH * 100) / totDrop : null;

  // 近一周最低
  const wkStart = now - WEEK_SEC;
  let lo: number | null = null;
  for (let i = firstGe(arr, wkStart); i < arr.length; i++) {
    if (lo === null || arr[i].p < lo) {
      lo = arr[i].p;
    }
  }
  meta.weekLow = lo;
}

function fmtDur(h: number): string {
  if (!(h > 0) || !Number.isFinite(h)) {
    return "--";
  }
  if (h < 1) {
    return "<1 小时";
  }
  if (h < 24) {
    return `${Math.round(h)} 小时`;
  }
  const d = h / 24;
  return d >= 10 ? `${Math.round(d)} 天` : `${d.toFixed(1)} 天`;
}

const avgText = computed(() => (meta.hpd == null ? "--" : `≈ ${fmtDur(meta.hpd)}`));
const avgNote = computed(() =>
  meta.hpd == null ? "近 30 天放电数据不足" : `近 30 天 ${meta.segs} 段放电实测推算`,
);
const estText = computed(() => {
  if (battery.percent === null || meta.hpd == null) {
    return "--";
  }
  if (battery.charging) {
    return "充电中";
  }
  return `≈ ${fmtDur((battery.percent / 100) * meta.hpd)}`;
});
const estNote = computed(() => {
  if (battery.percent === null) {
    return "设备未连接，暂无法推算";
  }
  if (meta.hpd == null) {
    return "近 30 天放电数据不足";
  }
  if (battery.charging) {
    return `拔线后满电可用 ≈ ${fmtDur(meta.hpd)}`;
  }
  return `按当前 ${battery.percent}% × 近 30 天实测推算`;
});
const weekLowText = computed(() => (meta.weekLow === null ? "--" : `${meta.weekLow}%`));
const weekLowNote = computed(() => (meta.weekLow === null ? "暂无采样" : "近 7 天实测"));

function refresh() {
  updateStats();
  scheduleDraw();
}

let tick = 0;
onMounted(() => {
  refresh();
  window.addEventListener("resize", scheduleDraw);
  tick = window.setInterval(() => {
    updateStats(); // 时间窗口随时间滑动，30s 同步一次
    scheduleDraw();
  }, 30_000);
});
onBeforeUnmount(() => {
  window.removeEventListener("resize", scheduleDraw);
  window.clearInterval(tick);
});
watch(() => samples.value, refresh);

// 开发自测钩子（生产构建剔除）：无头/节流环境下可手动触发重绘并检查 hover 状态
if (import.meta.env.DEV) {
  (window as unknown as Record<string, unknown>).__kbtoolHome = { draw, hover, tip };
}
</script>

<template>
  <div class="page">
    <h1>主页</h1>
    <div class="sub">98 键三模键盘 · {{ ifaceText }} · 每 15 秒自动记录（历史持久化在本机）</div>
    <div class="card">
      <div class="battery-hero">
        <div class="battery-ring">
          <svg width="150" height="150">
            <circle cx="75" cy="75" :r="R" fill="none" stroke="#222834" stroke-width="11" />
            <circle
              cx="75"
              cy="75"
              :r="R"
              fill="none"
              :stroke="ringColor"
              stroke-width="11"
              stroke-linecap="round"
              :stroke-dasharray="CIRC"
              :stroke-dashoffset="dashOffset"
              style="
                transition:
                  stroke-dashoffset 0.6s ease,
                  stroke 0.3s;
              "
            />
          </svg>
          <div class="pct">
            <b
              >{{ pctText
              }}<span v-if="battery.percent !== null" style="font-size: 20px">%</span></b
            ><span>剩余电量</span>
          </div>
        </div>
        <div class="battery-info">
          <div>
            <span class="badge" :class="battery.charging ? 'charge' : 'off'">{{
              chargeBadge
            }}</span>
          </div>
          <div style="font-size: 13px; line-height: 1.7; color: var(--dim)">
            电量由键盘固件电压估算，充电时读数会偏高，拔线后回落。<br />
            低电量（≤ 30%）将发送系统通知。
          </div>
          <div class="stats">
            <div class="stat">
              <div class="k">连接模式</div>
              <div class="v">{{ ifaceText }}</div>
            </div>
            <div class="stat">
              <div class="k">充电状态</div>
              <div class="v">{{ battery.charging ? "充电中" : "未充电" }}</div>
            </div>
            <div class="stat">
              <div class="k">近一周最低</div>
              <div class="v">{{ weekLowText }}</div>
              <div class="n">{{ weekLowNote }}</div>
            </div>
            <div class="stat hot">
              <div class="k">平均续航</div>
              <div class="v">{{ avgText }}</div>
              <div class="n">{{ avgNote }}</div>
            </div>
            <div class="stat hot" :class="{ charging: battery.charging }">
              <div class="k">预计续航</div>
              <div class="v">{{ estText }}</div>
              <div class="n">{{ estNote }}</div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <div class="card">
      <div class="chart-head">
        <h3 style="margin: 0">电量曲线（近一周）</h3>
        <div class="legend">
          <span><i class="dot g"></i>使用中</span>
          <span><i class="dot y"></i>充电</span>
        </div>
      </div>
      <div class="chart-box" @mousemove="onMove" @mouseleave="onLeave">
        <canvas ref="chart" class="chart"></canvas>
        <div
          v-show="tip.show"
          class="chart-tip"
          :class="{ below: !tip.above }"
          :style="{
            left: tip.left + 'px',
            top: tip.top + 'px',
            transform: tip.above ? 'translateY(-100%)' : 'none',
          }"
        >
          <div class="row">
            <b>{{ tip.p }}%</b>
            <span v-if="tip.c" class="chg">⚡ 充电中</span>
          </div>
          <div class="tm">{{ tip.time }}</div>
        </div>
      </div>
    </div>
  </div>
</template>
