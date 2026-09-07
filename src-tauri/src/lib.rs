mod device;
mod history;

use history::HistoryLog;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WebviewUrl};
use tauri_plugin_notification::NotificationExt;

use device::Battery;

/// 用户主动退出标志：托盘菜单"退出"置位后 exit()，ExitRequested 不再拦截
static QUITTING: AtomicBool = AtomicBool::new(false);

/// 开机自启注册时附加的命令行参数（写入注册表 Run 项），用于区分"开机拉起"与"手动启动"
const AUTOSTART_ARG: &str = "--autostart";

/// 静默启动配置路径：app_config/silent_start.json，内容 "1"=开启 "0"=关闭
fn silent_start_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("silent_start.json")
}

/// 读取静默启动开关；配置文件缺失视为开启（设置页默认勾选）
fn silent_start_enabled(app: &tauri::AppHandle) -> bool {
    std::fs::read_to_string(silent_start_path(app))
        .map(|s| s.trim() == "1")
        .unwrap_or(true)
}

/// 唤起主窗口：已存在则显示+聚焦，已销毁（关窗省内存模式）或未创建（静默启动）则重建
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    } else {
        let _ = tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("KBTool")
            .inner_size(1380.0, 860.0)
            .min_inner_size(1380.0, 860.0)
            .build();
    }
}

struct State {
    battery: Mutex<Option<Battery>>,
    /// 关窗行为：true = 直接退出程序；false = 销毁窗口、托盘常驻（设置页可切）
    close_quits: AtomicBool,
    /// 低电量通知阈值（默认 30%，设置页可切；Rust 侧持有，静默托盘模式也生效）
    notify_threshold: AtomicU8,
    /// 低电量通知去重标记：≤ 阈值提醒一次，回升到阈值+5 以上才复位
    notify_fired: AtomicBool,
    /// 电量历史日志（轮询线程写，命令读，同一把锁串行化）
    log: Mutex<HistoryLog>,
}

#[tauri::command]
fn battery_status(state: tauri::State<State>) -> Option<Battery> {
    state.battery.lock().ok()?.clone()
}

/// 读取持久化的电量历史：返回 since 之后（且最近 max_days 天内）的采样，升序
#[tauri::command]
fn battery_history(state: tauri::State<State>, since: u64, max_days: u32) -> Vec<history::Sample> {
    state
        .log
        .lock()
        .map(|l| l.read_since(since, u64::from(max_days)))
        .unwrap_or_default()
}

#[tauri::command]
fn set_close_action(state: tauri::State<State>, quit: bool) {
    state.close_quits.store(quit, Ordering::SeqCst);
}

/// 读取低电量通知阈值（设置页回显用）
#[tauri::command]
fn get_notify_threshold(state: tauri::State<State>) -> u8 {
    state.notify_threshold.load(Ordering::SeqCst)
}

/// 设置低电量通知阈值（上限 100）
#[tauri::command]
fn set_notify_threshold(state: tauri::State<State>, threshold: u8) {
    state.notify_threshold.store(threshold.min(100), Ordering::SeqCst);
}

/// 读取静默启动开关（前端设置页用；缺失即默认开启）
#[tauri::command]
fn get_silent_start(app: tauri::AppHandle) -> bool {
    silent_start_enabled(&app)
}

/// 持久化静默启动开关到 app_config/silent_start.json，下次开机自启时生效
#[tauri::command]
fn set_silent_start(app: tauri::AppHandle, silent: bool) -> Result<(), String> {
    std::fs::write(silent_start_path(&app), if silent { "1" } else { "0" })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_light(
    mode: u8,
    r: u8,
    g: u8,
    b: u8,
    colorful: bool,
    brightness: u8,
    speed: u8,
    direction: u8,
) -> Result<(), String> {
    device::send_cfg(&device::light_pkt(
        mode,
        [r, g, b],
        colorful,
        brightness,
        speed,
        direction,
    ))
}

#[tauri::command]
fn set_kbd_params(sleep_min: u8, respond_ms: u8) -> Result<(), String> {
    device::send_cfg(&device::params_pkt(sleep_min, respond_ms))
}

/// 后台轮询线程：设备在线时每 15s 查一次电量；离线时指数退避重试（2s→30s 封顶）
fn spawn_poller(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut backoff = Duration::from_secs(2);
        loop {
            if let Some((d, iface)) = device::open_device() {
                backoff = Duration::from_secs(2); // 连上即重置退避
                let mut ok = true;
                while ok {
                    match device::query_battery(&d) {
                        Some(pct) => {
                            let ts = history::now_secs();
                            let charging = device::wired_present();
                            // 持久化：先落盘再广播，窗口销毁/重启都不丢（文件读取与裁剪同锁串行）
                            if let Ok(mut log) = app.state::<State>().log.lock() {
                                log.append(pct, charging);
                            }
                            let b = Battery {
                                percent: pct,
                                iface,
                                charging,
                                t: ts,
                            };
                            let _ = app.emit("battery", &b);
                            // 低电量通知与 WebView 解耦（静默托盘 / 收起至托盘均生效）：
                            // 不充电且 ≤ 阈值时提醒一次，回升到阈值+5 以上才复位重报
                            let st = app.state::<State>();
                            if !b.charging
                                && b.percent <= st.notify_threshold.load(Ordering::SeqCst)
                                && !st.notify_fired.swap(true, Ordering::SeqCst)
                            {
                                let _ = app
                                    .notification()
                                    .builder()
                                    .title("键盘电量过低")
                                    .body(format!(
                                        "当前电量 {}%，请及时充电（仅支持电脑 USB 口充电）",
                                        pct
                                    ))
                                    .show();
                            } else if !b.charging
                                && b.percent
                                    > st.notify_threshold.load(Ordering::SeqCst).saturating_add(5)
                            {
                                st.notify_fired.store(false, Ordering::SeqCst);
                            }
                            // 托盘悬停显示实时电量
                            if let Some(tray) = app.tray_by_id("main-tray") {
                                let tip = if b.charging {
                                    format!("KBTool — 电量 {}%（充电中）", pct)
                                } else {
                                    format!("KBTool — 电量 {}%", pct)
                                };
                                let _ = tray.set_tooltip(Some(&tip));
                            }
                            if let Ok(mut g) = app.state::<State>().battery.lock() {
                                *g = Some(b);
                            }
                        }
                        None => ok = false, // 查询失败 -> 断开重连
                    }
                    if ok {
                        std::thread::sleep(Duration::from_secs(15));
                    }
                }
            }
            std::thread::sleep(backoff);
            if backoff < Duration::from_secs(30) {
                backoff *= 2;
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARG]), // 注册项始终带参数，开机拉起时据此区分“自启”与“手动启动”
        ))
        .setup(|app| {
            // 状态（含电量历史日志，路径在 app data 下）在 setup 里 resolve 后再 manage
            let log_path = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("battery_history.log");
            app.manage(State {
                battery: Mutex::new(None),
                close_quits: AtomicBool::new(false),
                notify_threshold: AtomicU8::new(30),
                notify_fired: AtomicBool::new(false),
                log: Mutex::new(HistoryLog::new(log_path)),
            });

            // 托盘：左键恢复窗口，菜单含退出
            let show = MenuItem::with_id(app, "show", "打开 KBTool", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::with_id("main-tray")
                .tooltip("KBTool — 键盘配置工具")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        QUITTING.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    "show" => show_main_window(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            spawn_poller(app.handle().clone());

            // 主窗口不再由 tauri.conf.json 自动创建，改为这里按需创建：
            // 静默启动（开机自启拉起 && 开关开启）时不建窗口，直接托盘驻留，省内存；
            // 手动双击启动 / 未开静默时照常弹出。
            let from_autostart = std::env::args().any(|a| a == AUTOSTART_ARG);
            if !(from_autostart && silent_start_enabled(app.handle())) {
                show_main_window(app.handle());
            }
            Ok(())
        })
        // 关窗行为（设置页可切）：
        // 收起 = 真销毁窗口（连带 WebView2 释放，内存 ~146MB→~25MB），托盘常驻；
        // 直接退出 = 置 QUITTING 后退出程序（设置页选了"直接退出"时）
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let quit = window
                    .app_handle()
                    .state::<State>()
                    .close_quits
                    .load(Ordering::SeqCst);
                if quit {
                    QUITTING.store(true, Ordering::SeqCst);
                    window.app_handle().exit(0);
                } else {
                    let _ = window.destroy();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            battery_status,
            battery_history,
            set_close_action,
            get_notify_threshold,
            set_notify_threshold,
            get_silent_start,
            set_silent_start,
            set_light,
            set_kbd_params
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // 托盘常驻：窗口销毁（省内存模式）不退出；仅托盘菜单"退出"置 QUITTING 后才放行
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}
