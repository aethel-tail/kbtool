//! 电量历史持久化
//!
//! 轮询线程每次成功查询后把 `时间戳,电量%,充电` 追加到 app data 下的日志文件，
//! 因此关窗（托盘常驻）与程序重启都不会丢数据；按天做一次过期裁剪，
//! 只保留最近 `KEEP_DAYS` 天，文件体积有界（≈MB 级）。
//! 前端通过 `battery_history` 命令读取增量/全量用于曲线与统计。

use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 日志保留天数（前端“近一月统计”取 30 天，留几天余量）
pub const KEEP_DAYS: u64 = 35;

/// 一条采样
#[derive(Clone, Copy, Serialize)]
pub struct Sample {
    /// unix 秒
    pub t: u64,
    /// 电量百分比 0-100
    pub p: u8,
    /// 是否检测到有线充电
    pub c: bool,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub struct HistoryLog {
    path: PathBuf,
    /// 追加句柄（懒打开、出错后置 None 下次重试）
    file: Option<File>,
    /// 上次裁剪所在的“天”，用于每日一次裁剪
    last_trim_day: u32,
}

impl HistoryLog {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            file: None,
            last_trim_day: 0,
        }
    }

    fn day(ts: u64) -> u32 {
        (ts / 86_400) as u32
    }

    fn open_append(&mut self) {
        if self.file.is_some() {
            return;
        }
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok();
    }

    /// 轮询线程调用（调用方需持有 State.log 锁）：追加一条采样 + 每日过期裁剪
    pub fn append(&mut self, pct: u8, charging: bool) {
        let ts = now_secs();
        if ts == 0 {
            return;
        }
        self.open_append();
        let mut failed = false;
        if let Some(f) = self.file.as_mut() {
            if writeln!(f, "{},{},{}", ts, pct, u8::from(charging)).is_err() {
                failed = true;
            }
        } else {
            failed = true;
        }
        if failed {
            self.file = None;
            return;
        }
        let day = Self::day(ts);
        if day != self.last_trim_day {
            self.last_trim_day = day;
            self.trim_old(ts);
        }
    }

    /// 头部数据早于保留窗口时整体重写（丢弃过期行）
    fn trim_old(&mut self, now: u64) {
        let cutoff = now.saturating_sub(KEEP_DAYS * 86_400);
        // 头部未过期则无需裁剪（只读首行，避免每天整读）
        let mut stale = false;
        if let Ok(f) = File::open(&self.path) {
            let mut lines = BufReader::new(f).lines();
            if let Some(Ok(first)) = lines.next() {
                if let Some(ts) = first.split(',').next().and_then(|v| v.parse::<u64>().ok()) {
                    stale = ts < cutoff;
                }
            }
        }
        if !stale {
            return;
        }
        let Ok(s) = fs::read_to_string(&self.path) else {
            return;
        };
        let mut keep = String::with_capacity(s.len() / 2);
        for line in s.lines() {
            if let Some(ts) = line.split(',').next().and_then(|v| v.parse::<u64>().ok()) {
                if ts >= cutoff {
                    keep.push_str(line);
                    keep.push('\n');
                }
            }
        }
        // 先关掉追加句柄再截断重写（Windows 上同进程双句柄会互相干扰）
        self.file = None;
        let _ = fs::write(&self.path, keep);
        self.open_append();
    }

    /// 读取 since 之后、最近 max_days 天内的采样（升序）。
    /// 文件为追加日志（≈MB 级），全量解析开销可忽略。
    pub fn read_since(&self, since: u64, max_days: u64) -> Vec<Sample> {
        let cutoff = now_secs().saturating_sub(max_days * 86_400);
        let Ok(f) = File::open(&self.path) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            let mut it = line.split(',');
            let (Some(ts), Some(p), Some(c)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            let (Ok(ts), Ok(p), Ok(c)) = (ts.parse::<u64>(), p.parse::<u8>(), c.parse::<u8>())
            else {
                continue;
            };
            if ts > since && ts >= cutoff {
                out.push(Sample {
                    t: ts,
                    p,
                    c: c != 0,
                });
            }
        }
        out
    }
}
