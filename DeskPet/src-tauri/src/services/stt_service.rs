use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_RECORD_SECS: f32 = 60.0;

/// 录音共享状态（不含任何 cpal 对象，保证 Send+Sync 可放 static）。
struct SharedRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
    stop_flag: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<Vec<u8>, String>>>>,
}

static RECORDER: Mutex<Option<Arc<SharedRecorder>>> = Mutex::new(None);

fn wav_from_f32(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(44 + samples.len() * 2);
    data.extend_from_slice(b"RIFF");
    let data_len = (samples.len() * 2) as u32;
    data.extend_from_slice(&(36 + data_len).to_le_bytes());
    data.extend_from_slice(b"WAVE");
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&channels.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * channels as u32 * 2;
    data.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align = channels * 2;
    data.extend_from_slice(&block_align.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        data.extend_from_slice(&v.to_le_bytes());
    }
    data
}

pub struct SttService;

impl SttService {
    /// 开始录音（默认输入设备）。已在录音时无副作用。
    pub fn start_recording() -> Result<(), String> {
        {
            let guard = RECORDER.lock().unwrap();
            if let Some(rec) = guard.as_ref() {
                if !rec.done.load(Ordering::SeqCst) {
                    return Ok(()); // 已在录音
                }
            }
        }

        let rec = Arc::new(SharedRecorder {
            samples: Arc::new(Mutex::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            done: Arc::new(AtomicBool::new(false)),
            result: Arc::new(Mutex::new(None)),
        });
        {
            let mut guard = RECORDER.lock().unwrap();
            *guard = Some(rec.clone());
        }

        std::thread::spawn(move || {
            let outcome = run_recording_thread(&rec);
            if let Err(e) = &outcome {
                tracing::error!("[STT] 录音线程失败: {}", e);
            }
            *rec.result.lock().unwrap() = Some(outcome);
            rec.done.store(true, Ordering::SeqCst);
        });
        Ok(())
    }

    /// 停止录音并返回 WAV 字节。
    pub fn stop_recording() -> Result<Vec<u8>, String> {
        let rec = {
            let guard = RECORDER.lock().unwrap();
            guard
                .as_ref()
                .cloned()
                .ok_or_else(|| "当前没有正在进行的录音".to_string())?
        };
        rec.stop_flag.store(true, Ordering::SeqCst);

        // 等待录音线程收尾（最多 3 秒）
        let deadline = Instant::now() + Duration::from_secs(3);
        while !rec.done.load(Ordering::SeqCst) {
            if Instant::now() > deadline {
                return Err("停止录音超时".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let result = rec.result.lock().unwrap().take();
        // 清理状态，允许下一次录音
        {
            let mut guard = RECORDER.lock().unwrap();
            *guard = None;
        }
        result.ok_or_else(|| "录音结果丢失".to_string())?
    }

    /// 调 OpenAI 兼容 whisper 端点转写语音（multipart: file + model）。
    pub async fn transcribe(
        wav: Vec<u8>,
        endpoint: &str,
        api_key: &str,
        model: &str,
    ) -> Result<String, String> {
        let url = format!("{}/audio/transcriptions", endpoint.trim_end_matches('/'));
        let part = reqwest::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| e.to_string())?;
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", model.to_string());

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(&url)
            .bearer_auth(api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("语音转写请求失败: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| format!("读取转写响应失败: {e}"))?;
        if !status.is_success() {
            let snippet: String = body.chars().take(200).collect();
            return Err(format!("语音转写失败 ({}): {}", status, snippet));
        }

        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("解析转写响应失败: {e}"))?;
        json["text"]
            .as_str()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or("转写结果为空".into())
    }
}

/// 录音线程：cpal 对象全部为线程局部，通过原子标志停止。
fn run_recording_thread(rec: &SharedRecorder) -> Result<Vec<u8>, String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("未找到麦克风设备，请检查麦克风连接")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("获取麦克风配置失败: {e}"))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let max_samples = (sample_rate as f32 * MAX_RECORD_SECS * channels as f32) as usize;

    let err_fn = |e| tracing::error!("[STT] 录音流错误: {}", e);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let s = Arc::clone(&rec.samples);
            device
                .build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        s.lock().unwrap().extend_from_slice(data);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("启动录音失败: {e}"))?
        }
        cpal::SampleFormat::I16 => {
            let s = Arc::clone(&rec.samples);
            device
                .build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        let mut buf = s.lock().unwrap();
                        buf.extend(data.iter().map(|v| *v as f32 / i16::MAX as f32));
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("启动录音失败: {e}"))?
        }
        cpal::SampleFormat::U16 => {
            let s = Arc::clone(&rec.samples);
            device
                .build_input_stream(
                    &config.into(),
                    move |data: &[u16], _| {
                        let mut buf = s.lock().unwrap();
                        buf.extend(data.iter().map(|v| (*v as f32 - 32768.0) / 32768.0));
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("启动录音失败: {e}"))?
        }
        other => return Err(format!("不支持的采样格式: {:?}", other)),
    };

    stream.play().map_err(|e| format!("启动录音失败: {e}"))?;

    // 等待停止标志或达到时长上限
    while !rec.stop_flag.load(Ordering::SeqCst) {
        let len = rec.samples.lock().unwrap().len();
        if len >= max_samples {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // drop stream 即停止录音（cpal 无 stop 方法）
    drop(stream);

    let samples = rec.samples.lock().unwrap().clone();
    let seconds = samples.len() as f32 / (sample_rate as f32 * channels as f32);
    if seconds < 0.3 || samples.is_empty() {
        return Err("录音太短，请至少说 1 秒再试".into());
    }
    Ok(wav_from_f32(&samples, sample_rate, channels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header_format() {
        // 1 秒 16000Hz 单声道正弦波
        let samples: Vec<f32> = (0..16000)
            .map(|i| (i as f32 / 16000.0 * 440.0 * 6.283).sin())
            .collect();
        let wav = wav_from_f32(&samples, 16000, 1);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[20..22], &1u16.to_le_bytes()); // PCM
        assert_eq!(&wav[22..24], &1u16.to_le_bytes()); // mono
        assert_eq!(&wav[24..28], &16000u32.to_le_bytes()); // rate
        assert_eq!(&wav[34..36], &16u16.to_le_bytes()); // bits
        assert_eq!(&wav[36..40], b"data");
        let data_len = wav.len() - 44;
        assert_eq!(data_len, samples.len() * 2);
        assert_eq!(&wav[40..44], &(data_len as u32).to_le_bytes());
    }
}
