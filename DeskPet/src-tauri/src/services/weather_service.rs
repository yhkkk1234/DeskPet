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

    let resp = client.get(&url).send().await.map_err(|e| format!("天气API请求失败: {}", e))?;

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
            eprintln!("[天气] 刷新失败: {}", e);
            let locked = cache.lock().ok()?;
            locked.info.as_ref().map(|w| format_weather_context(w))
        }
    }
}

fn format_weather_context(info: &WeatherInfo) -> String {
    format!("当前天气：{}，{}，{:.0}°C，湿度{}%", info.city, info.description, info.temperature, info.humidity)
}
