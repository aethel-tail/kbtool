# KBTool

某 98 键三模键盘的 Windows 桌面配置工具：**电量监控、灯光控制、休眠/响应参数调节**。

基于 Tauri 2 + Vue 3 + TypeScript 构建，Rust 侧通过 HID 协议与键盘/2.4G 接收器通信（协议逆向自官方驱动）。**非官方工具，与任何键盘厂商无关。**

## 功能

- 电量实时监控（托盘悬停显示电量）、低电量系统通知
- 电量历史持久化：近一周曲线、近一月续航统计（平均/预计续航）
- 灯光控制：19 种灯效模式、亮度/速度/方向/颜色（多彩）
- 键盘参数：休眠档位（0-3）、按键响应档位（1-5，防连击）
- 托盘常驻（关窗省内存）、开机自启可选

> ⚠ 蓝牙模式下固件不开放配置通道；请使用 2.4G 接收器或 USB 有线连接。

## 开发

```bash
pnpm install          # 安装依赖
pnpm tauri dev        # 开发调试
pnpm tauri build      # 打包发布
```

## 项目结构

- `src/` — Vue 前端（电量图表 / 灯光 / 键盘设置）
- `src-tauri/` — Rust 后端（HID 协议层、电量轮询、历史持久化、托盘）
- `docs/操作指南.md` — 使用说明
- `app-icon.svg` — 应用图标源图，重新生成全套图标：`pnpm tauri icon app-icon.svg`

## 许可

Copyright (c) 2026 [aethel-tail](https://github.com/aethel-tail)，以 [MIT](./LICENSE) 协议开源。

Fork / 二次开发 / 分发时请保留原作者署名与本 LICENSE 文件。
