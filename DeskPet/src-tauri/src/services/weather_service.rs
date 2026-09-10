use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConfig {
    pub api_key: String,
    pub city: String,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            city: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherInfo {
    pub description: String,
    pub temperature: f64,
    pub humidity: u32,
    pub city: String,
}

pub struct WeatherCache {
    pub info: Option<WeatherInfo>,
    pub last_fetch: Option<DateTime<Utc>>,
    pub config: WeatherConfig,
}

impl Default for WeatherCache {
    fn default() -> Self {
        Self {
            info: None,
            last_fetch: None,
            config: WeatherConfig::default(),
        }
    }
}

const WEATHER_STALE_SECS: i64 = 1800;

impl WeatherCache {
    pub fn is_stale(&self) -> bool {
        match self.last_fetch {
            Some(t) => (Utc::now() - t).num_seconds() >= WEATHER_STALE_SECS,
            None => true,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.config.api_key.is_empty() && !self.config.city.is_empty()
    }
}

#[derive(Deserialize)]
struct OpenWeatherResponse {
    weather: Vec<OpenWeatherCondition>,
    main: OpenWeatherMain,
    name: String,
}

#[derive(Deserialize)]
struct OpenWeatherCondition {
    description: String,
}

#[derive(Deserialize)]
struct OpenWeatherMain {
    temp: f64,
    humidity: u32,
}

/// 把 reqwest 错误转成「可安全写进日志」的字符串。
///
/// reqwest 会把完整请求 URL 附在错误末尾（` for url (...)`），而 OpenWeather 的
/// appid 是 URL 查询参数——一旦请求失败，错误经 `fetch_weather` 返回到
/// `refresh_weather_if_stale` 里的 `tracing::error!`，就会把密钥明文写进
/// `%APPDATA%/deskpet/logs/`，与 README「API Key 加密存储」的承诺相矛盾。
///
/// 两道处理：剥掉 URL（`without_url`），再正则兜底掩掉可能残留的 appid=xxx。
fn redact_api_key(e: reqwest::Error) -> String {
    let without_url = e.without_url().to_string();
    redact_appid(&without_url)
}

/// 把 `appid=<值>` 掩码成 `appid=****`（保留字段名，便于排查是参数问题还是网络问题）。
///
/// 扫描到「查询参数分隔符」为止（`&`、空白、右括号、引号），而不是「非字母数字为止」——
/// 后者遇到含 `+` `/` `=` 的 base64 型密钥会只掩一半、把剩余密钥留在日志里。
/// 宁可多掩（如 `%2F` 转义也一并吃掉）也不能少掩。
fn redact_appid(s: &str) -> String {
    const DELIM: [char; 5] = ['&', ' ', ')', '"', '\''];
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find("appid=") {
        out.push_str(&rest[..pos + "appid=".len()]);
        out.push_str("****");
        let after = &rest[pos + "appid=".len()..];
        let end = after.find(|c: char| DELIM.contains(&c)).unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

pub async fn fetch_weather(config: &WeatherConfig) -> Result<WeatherInfo, String> {
    let url = format!(
        "https://api.openweathermap.org/data/2.5/weather?q={}&appid={}&lang=zh_cn&units=metric",
        urlencoding::encode(&config.city),
        config.api_key,
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("天气API请求失败: {}", redact_api_key(e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("天气API返回错误 {}: {}", status, body));
    }

    let data: OpenWeatherResponse = resp.json().await.map_err(|e| format!("解析天气数据失败: {}", e))?;

    let condition = data.weather.first().map(|w| w.description.clone()).unwrap_or_else(|| "未知".to_string());

    Ok(WeatherInfo {
        description: condition,
        temperature: data.main.temp,
        humidity: data.main.humidity,
        city: data.name,
    })
}

pub async fn refresh_weather_if_stale(cache: &Mutex<WeatherCache>) -> Option<String> {
    let (should_fetch, config) = {
        let locked = cache.lock().ok()?;
        if !locked.is_configured() {
            return None;
        }
        (locked.is_stale(), locked.config.clone())
    };

    if !should_fetch {
        let locked = cache.lock().ok()?;
        return locked.info.as_ref().map(|w| format_weather_context(w));
    }

    match fetch_weather(&config).await {
        Ok(info) => {
            let context = format_weather_context(&info);
            if let Ok(mut locked) = cache.lock() {
                locked.info = Some(info);
                locked.last_fetch = Some(Utc::now());
            }
            Some(context)
        }
        Err(e) => {
            tracing::error!("[天气] 刷新失败: {}", e);
            let locked = cache.lock().ok()?;
            locked.info.as_ref().map(|w| format_weather_context(w))
        }
    }
}

fn format_weather_context(info: &WeatherInfo) -> String {
    format!("当前天气：{}，{}，{:.0}°C，湿度{}%", info.city, info.description, info.temperature, info.humidity)
}

#[cfg(test)]
mod tests {
    use super::redact_appid;

    /// 回归保护：天气密钥绝不能明文进日志。
    #[test]
    fn test_redact_appid_masks_key() {
        let err = "error sending request for url (https://api.openweathermap.org/data/2.5/weather?q=beijing&appid=abcdef0123456789abcdef0123456789&lang=zh_cn)";
        let out = redact_appid(err);
        assert!(!out.contains("abcdef0123456789"), "密钥仍出现在脱敏结果中: {out}");
        assert!(out.contains("appid=****"), "应保留 appid=**** 便于排查: {out}");
        // 其余排查信息（城市、网络错误原因）必须保留
        assert!(out.contains("q=beijing"), "不应误删其他参数: {out}");
        assert!(out.contains("error sending request"), "不应误删错误原因: {out}");
    }

    #[test]
    fn test_redact_appid_without_key_is_unchanged() {
        let s = "天气API请求失败: connection timed out";
        assert_eq!(redact_appid(s), s);
    }

    #[test]
    fn test_redact_appid_multiple_and_trailing() {
        assert_eq!(redact_appid("appid=SECRET123"), "appid=****");
        assert_eq!(redact_appid("a=1&appid=X9&b=2"), "a=1&appid=****&b=2");
        // 以分隔符为界：%2F 等转义也一并吃掉（宁可多掩，不可少掩）
        assert_eq!(redact_appid("appid=ab%2Fcd&x=1"), "appid=****&x=1");
        // 多个 appid 一起掩
        assert_eq!(redact_appid("appid=A1&appid=B2"), "appid=****&appid=****");
    }

    /// 关键性质：绝不能只掩一部分而把密钥残片留在日志里。
    /// 覆盖含 + / = 的 base64 型密钥（旧实现会在第一个非字母数字处停下）。
    #[test]
    fn test_redact_appid_never_partially_masks() {
        let base64ish = "Ab+C/dEf=1234567890";
        let out = redact_appid(&format!("appid={base64ish}&lang=zh_cn"));
        assert!(!out.contains("Ab+C"), "密钥残片仍在: {out}");
        assert!(!out.contains("dEf"), "密钥残片仍在: {out}");
        assert!(out.contains("lang=zh_cn"), "后继参数被误删: {out}");
        // reqwest 的 URL 包裹形式：以 ')' 收尾也必须被吃掉
        let wrapped = redact_appid("for url (https://api.openweathermap.org/data/2.5/weather?q=beijing&appid=deadbeef0123456789abcdef01234567)");
        assert!(!wrapped.contains("deadbeef"), "密钥残片仍在: {wrapped}");
    }
}
