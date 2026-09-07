// Material Design 水波纹（轻量版，跟手式）
// 全局事件委托：pointerdown 在点击处生成涟漪并展开（~300ms），
// pointerup/cancel 后淡出（~450ms）并移除。
// GPU 友好：全程 Web Animations API 只驱动 transform/opacity（合成器属性）
const HIT = "nav a, .key, .mode, .opt, .toggle, button";

export function initRipple(): void {
  document.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) {
      return;
    } // 仅左键
    const el = (e.target as Element | null)?.closest<HTMLElement>(HIT);
    if (!el) {
      return;
    }
    const r = el.getBoundingClientRect();
    const d = Math.max(r.width, r.height) * 2.2; // 直径保证覆盖整个元素
    const s = document.createElement("span");
    s.className = "ripple";
    Object.assign(s.style, {
      width: `${d}px`,
      height: `${d}px`,
      left: `${e.clientX - r.left - d / 2}px`,
      top: `${e.clientY - r.top - d / 2}px`,
    });
    el.appendChild(s);

    let done = false;
    const finish = () => {
      if (done) {
        return;
      }
      done = true;
      const fade = s.animate([{ opacity: 0.3 }, { opacity: 0 }], {
        duration: 450,
        easing: "ease-out",
      });
      fade.onfinish = () => s.remove();
    };
    // 按下即展开，抬起（或兜底 2s）后淡出；once 自动解绑不泄漏
    window.addEventListener("pointerup", finish, { once: true });
    window.addEventListener("pointercancel", finish, { once: true });
    setTimeout(finish, 2000);
    s.animate([{ transform: "scale(0)" }, { transform: "scale(1)" }], {
      duration: 300,
      easing: "cubic-bezier(.2,.6,.35,1)",
      fill: "forwards",
    });
  });
}
