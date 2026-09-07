//! VTER Galaxy100 HID 协议层（协议逆向自原版驱动，见仓库 findings.md）
//!
//! 电量查询: 33B 输出报告 `00 20 01 00*30 21` -> 32B 输入报告
//!   响应 byte[3] = 电量%, 末字节 = 前 31 字节累加和校验
//! 接口: 2.4G -> 05AC:024F MI_03 ; USB 有线 -> 0C45:8006 MI_00

use hidapi::{HidApi, HidDevice};
use serde::Serialize;

pub const VID_DONGLE: u16 = 0x05AC;
pub const PID_DONGLE: u16 = 0x024F;
pub const VID_USB: u16 = 0x0C45;
pub const PID_USB: u16 = 0x8006;

#[derive(Clone, Serialize)]
pub struct Battery {
    pub percent: u8,
    pub iface: &'static str,
    pub charging: bool,
    /// 采样时间（unix 秒），由轮询线程填充，供前端历史去重/对齐
    pub t: u64,
}

/// 打开配置通道：优先 2.4G 接收器，其次 USB 有线。返回 (设备, 接口名)。
pub fn open_device() -> Option<(HidDevice, &'static str)> {
    let api = HidApi::new().ok()?;
    for dev in api.device_list() {
        if dev.vendor_id() == VID_DONGLE
            && dev.product_id() == PID_DONGLE
            && dev.interface_number() == 3
        {
            return dev.open_device(&api).ok().map(|d| (d, "2.4G"));
        }
    }
    for dev in api.device_list() {
        if dev.vendor_id() == VID_USB && dev.product_id() == PID_USB && dev.interface_number() == 0
        {
            return dev.open_device(&api).ok().map(|d| (d, "USB"));
        }
    }
    None
}

/// 有线设备（0C45:8006）是否出现在系统枚举中 → 充电启发式检测
pub fn wired_present() -> bool {
    HidApi::new()
        .map(|api| {
            api.device_list()
                .any(|d| d.vendor_id() == VID_USB && d.product_id() == PID_USB)
        })
        .unwrap_or(false)
}

/// 查询一次电量，返回百分比（校验失败/超时返回 None）
pub fn query_battery(dev: &HidDevice) -> Option<u8> {
    let mut q = [0u8; 33];
    q[1] = 0x20; // cmd
    q[2] = 0x01; // sub: battery
    q[32] = 0x21; // checksum: 0x20+0x01
    dev.write(&q).ok()?;

    let mut buf = [0u8; 64];
    let n = dev.read_timeout(&mut buf, 3000).ok()?;
    if n < 32 || buf[0] != 0x20 || buf[1] != 0x01 {
        return None;
    }
    let sum: u8 = buf[..31].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    if sum != buf[31] {
        return None;
    }
    Some(buf[3])
}

// ===== 配置写入（Phase 4 逆向，见 findings.md §8-10）=====
// 33B 写包: [0]=0 [1]=cmd [2]=sub [3..32]=29B 体 [32]=前31字节累加校验
// 灯光 cmd 05 sub 10: idx4=模式(1-19,0=关灯) idx5-7=RGB idx12=多彩 idx13=亮度1-5 idx14=速度1-5 idx15=方向 idx18-19=aa55
// 参数 cmd 07 sub 10: idx5=01 idx10=休眠档0-3 idx12=响应档1-5 idx18-19=aa55

fn build_pkt(cmd: u8, sub: u8, body: &[u8; 29]) -> [u8; 33] {
    let mut p = [0u8; 33];
    p[1] = cmd;
    p[2] = sub;
    p[3..32].copy_from_slice(body);
    p[32] = p[..31].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    p
}

pub fn light_pkt(
    mode: u8,
    rgb: [u8; 3],
    colorful: bool,
    brightness: u8,
    speed: u8,
    direction: u8,
) -> [u8; 33] {
    let mut body = [0u8; 29];
    body[1] = mode; // idx4
    body[2..5].copy_from_slice(&rgb); // idx5-7
    body[9] = u8::from(colorful); // idx12
    body[10] = brightness; // idx13
    body[11] = speed; // idx14
    body[12] = direction; // idx15
    body[15] = 0xaa;
    body[16] = 0x55; // idx18-19 标记
    build_pkt(0x05, 0x10, &body)
}

pub fn params_pkt(sleep_min: u8, respond_ms: u8) -> [u8; 33] {
    let mut body = [0u8; 29];
    body[2] = 0x01; // idx5
    body[7] = sleep_min; // idx10 休眠档 0-3
    body[9] = respond_ms; // idx12 响应档 1-5
    body[15] = 0xaa;
    body[16] = 0x55;
    build_pkt(0x07, 0x10, &body)
}

/// 发 33B 配置包：独立开句柄 → 会话初始化(00 02 00) → 发包。
/// 独立句柄避免与轮询线程共享；init 幂等（实测每命令前 init 均可）
fn open_mi03(api: &HidApi) -> Result<HidDevice, String> {
    for d in api.device_list() {
        if d.vendor_id() == VID_DONGLE && d.product_id() == PID_DONGLE && d.interface_number() == 3
        {
            return d.open_device(api).map_err(|e| e.to_string());
        }
    }
    Err("键盘未连接（2.4G 接收器不在线）".into())
}

pub fn send_cfg(pkt: &[u8; 33]) -> Result<(), String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let dev = open_mi03(&api)?;
    // 会话初始化 00 02 00（裸发配置包键盘只回 echo 不执行）
    let mut init = [0u8; 33];
    init[1] = 0x02;
    init[32] = 0x02;
    dev.write(&init).map_err(|e| e.to_string())?;
    let _ = dev.read_timeout(&mut [0u8; 64], 500); // 吃掉 init 回包
    dev.write(pkt).map_err(|e| e.to_string())?;
    Ok(())
}
