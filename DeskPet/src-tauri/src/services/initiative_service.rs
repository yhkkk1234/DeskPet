use chrono::{DateTime, Local, Timelike, Utc};
use serde::{Deserialize, Serialize};

/// 主动搭话配置（由设置页控制，剪贴板感知默认关闭以保护隐私）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeConfig {
    pub night_greeting: bool,
    pub clipboard_sense: bool,
    pub battery_alert: bool,
}

impl Default for InitiativeConfig {
    fn default() -> Self {
        Self {
            night_greeting: true,
            clipboard_sense: false,
            battery_alert: true,
        }
    }
}

/// 各触发源的时间/状态追踪（冷却控制）
#[derive(Debug, Clone)]
pub struct InitiativeState {
    pub last_night_greeting_date: Option<chrono::NaiveDate>,
    pub last_battery_alert: DateTime<Utc>,
    pub last_clipboard_check: DateTime<Utc>,
    pub last_clipboard_alert: DateTime<Utc>,
    pub last_clipboard_text: Option<String>,
}

impl Default for InitiativeState {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            last_night_greeting_date: None,
            last_battery_alert: now,
            last_clipboard_check: now,
            last_clipboard_alert: now,
            last_clipboard_text: None,
        }
    }
}

/// 一次触发对应一种场景
#[derive(Debug, Clone)]
pub enum InitiativeTrigger {
    NightGreeting,
    BatteryAlert { percent: u8 },
    ClipboardSense { text: String },
}

impl InitiativeTrigger {
    pub fn scene_description(&self) -> String {
        match self {
            InitiativeTrigger::NightGreeting => "现在是深夜，主人还没睡，还守在电脑前".to_string(),
            InitiativeTrigger::BatteryAlert { percent } => {
                format!("主人的笔记本电脑电量只剩 {}%，还没插电源", percent)
            }
            InitiativeTrigger::ClipboardSense { text } => {
                let trimmed: String = text.chars().take(40).collect();
                if text.len() > 40 {
                    format!("主人刚才复制了一段内容：{}…", trimmed)
                } else {
                    format!("主人刚才复制了：{}", trimmed)
                }
            }
        }
    }

    /// AI 生成失败时的兜底文案（口语化、符合人设）
    pub fn fallback_text(&self, love_hate: f64) -> String {
        match self {
            InitiativeTrigger::NightGreeting => {
                if love_hate > 20.0 {
                    "这么晚还没睡呀？我陪你一会儿~".to_string()
                } else {
                    "夜深了，还不睡吗？".to_string()
                }
            }
            InitiativeTrigger::BatteryAlert { percent } => {
                format!("诶，电量只剩 {}% 了，记得充电哦", percent)
            }
            InitiativeTrigger::ClipboardSense { .. } => {
                "你刚复制的东西看起来挺有意思的".to_string()
            }
        }
    }
}

pub struct InitiativeService;

impl InitiativeService {
    /// 检查是否应主动搭话（一次最多返回一个触发源，按优先级：深夜 > 电量 > 剪贴板）
    pub fn check_triggers(
        state: &mut InitiativeState,
        config: &InitiativeConfig,
    ) -> Option<InitiativeTrigger> {
        let now = Utc::now();

        if config.night_greeting {
            let today = Local::now().date_naive();
            let hour = Local::now().hour();
            let is_late_night = hour >= 22 || hour < 2;
            let already_greeted = state.last_night_greeting_date == Some(today);
            if is_late_night && !already_greeted {
                state.last_night_greeting_date = Some(today);
                return Some(InitiativeTrigger::NightGreeting);
            }
        }

        if config.battery_alert {
            let cooldown_ok = (now - state.last_battery_alert).num_minutes() >= 30;
            if cooldown_ok {
                if let Some((percent, on_ac)) = Self::battery_status() {
                    if !on_ac && percent <= 20 {
                        state.last_battery_alert = now;
                        return Some(InitiativeTrigger::BatteryAlert { percent });
                    }
                }
            }
        }

        if config.clipboard_sense {
            let check_ok = (now - state.last_clipboard_check).num_seconds() >= 60;
            let alert_ok = (now - state.last_clipboard_alert).num_minutes() >= 10;
            if check_ok && alert_ok {
                state.last_clipboard_check = now;
                if let Some(text) = Self::read_clipboard() {
                    let changed = state.last_clipboard_text.as_deref() != Some(text.as_str());
                    let interesting = Self::is_interesting_text(&text);
                    state.last_clipboard_text = Some(text.clone());
                    if changed && interesting {
                        state.last_clipboard_alert = now;
                        return Some(InitiativeTrigger::ClipboardSense { text });
                    }
                }
            }
        }

        None
    }

    /// 剪贴板内容是否"值得搭话"：URL 或长度 >= 40 的文本
    fn is_interesting_text(text: &str) -> bool {
        let t = text.trim();
        if t.is_empty() {
            return false;
        }
        t.starts_with("http://") || t.starts_with("https://") || t.chars().count() >= 40
    }

    /// 电池状态：(电量百分比 0-100, 是否外接电源)。无法获取时返回 None。
    /// 台式机（一直插电）返回 on_ac=true，永远不会触发低电量提醒。
    pub fn battery_status() -> Option<(u8, bool)> {
        #[repr(C)]
        struct SystemPowerStatus {
            acline_status: u8,
            battery_flag: u8,
            battery_life_percent: u8,
            system_status_flag: u8,
            battery_life_time: u32,
            battery_full_life_time: u32,
        }
        extern "system" {
            fn GetSystemPowerStatus(lp_system_power_status: *mut SystemPowerStatus) -> i32;
        }

        let mut status = SystemPowerStatus {
            acline_status: 0,
            battery_flag: 0,
            battery_life_percent: 0,
            system_status_flag: 0,
            battery_life_time: 0,
            battery_full_life_time: 0,
        };
        let result = unsafe { GetSystemPowerStatus(&mut status) };
        if result == 0 {
            return None;
        }
        // 255 = 未知
        if status.battery_life_percent == 255 {
            return None;
        }
        // ACLineStatus: 0 = 电池供电, 1 = 外接电源, 255 = 未知
        let on_ac = match status.acline_status {
            0 => false,
            1 => true,
            _ => return None,
        };
        Some((status.battery_life_percent, on_ac))
    }

    /// 读取剪贴板纯文本（UTF-16 → String）。剪贴板被占用/非文本时返回 None。
    pub fn read_clipboard() -> Option<String> {
        extern "system" {
            fn OpenClipboard(h_wnd_new_owner: isize) -> i32;
            fn CloseClipboard() -> i32;
            fn GetClipboardData(u_format: u32) -> isize;
            fn GlobalLock(h_mem: isize) -> *mut u16;
            fn GlobalUnlock(h_mem: isize) -> i32;
            fn GetClipboardSequenceNumber() -> u32;
        }
        const CF_UNICODETEXT: u32 = 13;

        let _seq = unsafe { GetClipboardSequenceNumber() };
        if unsafe { OpenClipboard(0) } == 0 {
            return None;
        }
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
        let result = if handle == 0 {
            None
        } else {
            let ptr = unsafe { GlobalLock(handle) };
            if ptr.is_null() {
                None
            } else {
                let mut len = 0usize;
                unsafe {
                    while *ptr.add(len) != 0 {
                        len += 1;
                    }
                }
                let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                let text = String::from_utf16_lossy(slice);
                unsafe { GlobalUnlock(handle) };
                if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
        };
        unsafe { CloseClipboard() };
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_has_clipboard_off() {
        let config = InitiativeConfig::default();
        assert!(config.night_greeting);
        assert!(config.battery_alert);
        assert!(!config.clipboard_sense);
    }

    #[test]
    fn test_interesting_text_rules() {
        assert!(InitiativeService::is_interesting_text("https://example.com/abc"));
        assert!(InitiativeService::is_interesting_text("http://localhost:3000"));
        let long = "这是一个超过四十个字的文本，用于测试剪贴板感知功能是否会正确识别长文本并触发搭话";
        assert!(InitiativeService::is_interesting_text(long));
        assert!(!InitiativeService::is_interesting_text(""));
        assert!(!InitiativeService::is_interesting_text("   "));
        assert!(!InitiativeService::is_interesting_text("短文本"));
    }

    #[test]
    fn test_clipboard_cooldown_respected() {
        let mut state = InitiativeState::default();
        // 必须显式关闭其他触发源：check_triggers 按 深夜 → 电量 → 剪贴板 顺序短路返回。
        // 原写法用 `..Default::default()`，而默认 night_greeting=true / battery_alert=true，
        // 导致本测试在 22:00~02:00（is_late_night 为真）或未插电且电量≤20% 时必然失败
        // —— 与剪贴板冷却逻辑无关的环境依赖型 flaky。
        let config = InitiativeConfig {
            night_greeting: false,
            clipboard_sense: true,
            battery_alert: false,
        };
        // 初始 now，剪贴板 check 刚发生（默认 now），60s 内不应再检查
        state.last_clipboard_check = Utc::now();
        assert!(InitiativeService::check_triggers(&mut state, &config).is_none());
    }

    #[test]
    fn test_trigger_priority_night_first() {
        // 剪贴板关闭、电量关闭，只测深夜逻辑的日期记录
        let mut state = InitiativeState::default();
        let config = InitiativeConfig {
            night_greeting: true,
            clipboard_sense: false,
            battery_alert: false,
        };
        let hour = Local::now().hour();
        let is_late = hour >= 22 || hour < 2;
        // 深夜时第一次应触发；非深夜不应触发
        let first = InitiativeService::check_triggers(&mut state, &config);
        assert_eq!(first.is_some(), is_late);
        // 无论是否触发，第二次（同一天）都不应再触发
        let second = InitiativeService::check_triggers(&mut state, &config);
        assert!(second.is_none());
    }

    #[test]
    fn test_fallback_text_exists() {
        let t1 = InitiativeTrigger::NightGreeting;
        let t2 = InitiativeTrigger::BatteryAlert { percent: 15 };
        let t3 = InitiativeTrigger::ClipboardSense { text: "xxx".into() };
        assert!(!t1.fallback_text(50.0).is_empty());
        assert!(!t2.fallback_text(0.0).is_empty());
        assert!(!t3.fallback_text(-10.0).is_empty());
    }
}
