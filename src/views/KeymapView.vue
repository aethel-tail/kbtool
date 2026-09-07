<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from "vue";
import { KEYS, CANVAS, SCALE, isTwoLine } from "../lib/layout";

// 纯展示模式：点击仅高亮，不做映射（键映射/宏协议开发复杂度高，暂不提供，见 README）
const selected = ref<string | null>(null);

// 键位图自适应：容器不足 1040px 时等比缩小，保证永不横向溢出
const wrap = ref<HTMLElement>();
const fitScale = ref(1);
function fit() {
  const el = wrap.value;
  if (!el) {
    return;
  }
  const avail = el.clientWidth - 16; // 减 .kb-wrap 左右 padding 8*2
  fitScale.value = Math.min(1, avail / (CANVAS.w * SCALE));
}
function onWinResize() {
  fit();
}
onMounted(() => {
  nextTick(fit);
  window.addEventListener("resize", onWinResize);
});
onBeforeUnmount(() => window.removeEventListener("resize", onWinResize));

const effScale = computed(() => SCALE * fitScale.value);

function label(name: string): string {
  return isTwoLine(name) ? name.slice(0, 1) + "\n" + name.slice(1) : name;
}
</script>

<template>
  <div class="page">
    <h1>按键映射</h1>
    <div class="sub">键位布局示意 · 点击高亮预览</div>
    <div class="card">
      <div class="kb-wrap" ref="wrap">
        <div
          class="kb"
          :style="{ width: CANVAS.w * effScale + 'px', height: CANVAS.h * effScale + 'px' }"
        >
          <div
            v-for="k in KEYS"
            :key="k.name + k.x + k.y"
            class="key"
            :class="{ sel: selected === k.name }"
            :style="{
              left: k.x * effScale + 'px',
              top: k.y * effScale + 'px',
              width: k.w * effScale + 'px',
              height: k.h * effScale + 'px',
            }"
            @click="selected = k.name"
          >
            {{ label(k.name) }}
          </div>
        </div>
      </div>
      <div class="hint" style="margin-top: 18px">
        <b>自定义映射暂不支持。</b>宏/重映射协议复杂度超出本工具范围（官方驱动可完成此功能）；
        若未来有人贡献协议逆向或官方开放接口，将在此提供。<br />
        键位数据（101 键坐标）已从官方 layouts XML 复刻，仅作布局示意。
      </div>
    </div>
  </div>
</template>
