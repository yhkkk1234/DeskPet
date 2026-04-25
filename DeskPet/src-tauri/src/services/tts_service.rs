use reqwest::Client;

pub struct EdgeTTS {
    client: Client,
}

/// Available Chinese voices on Microsoft Edge TTS (free)
pub const EDGE_VOICES: &[(&str, &str)] = &[
    ("zh-CN-XiaoxiaoNeural", "晓晓 (女/活泼)"),
    ("zh-CN-XiaoyiNeural", "晓伊 (女/温柔)"),
    ("zh-CN-YunjianNeural", "云健 (男/阳光)"),
    ("zh-CN-YunxiNeural", "云希 (男/沉稳)"),
    ("zh-CN-YunxiaNeural", "云霞 (女/亲切)"),
    ("zh-CN-YunyangNeural", "云扬 (男/新闻)"),
    ("zh-CN-XiaochenNeural", "晓晨 (女/自然)"),
    ("zh-CN-XiaohanNeural", "晓涵 (女/甜美)"),
];

impl EdgeTTS {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Synthesize speech and return audio bytes (MP3 format)
    pub async fn synthesize(
        &self,
        text: &str,
        voice: &str,
    ) -> Result<Vec<u8>, String> {
        let endpoint = "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1\
            ?TrustedClientToken=6A5AA1D4EAFF4E9FB37E23D68491D6F4";

        let ssml = format!(
            "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='zh-CN'>\
             <voice name='{}'>{}</voice></speak>",
            voice, escape_xml(text)
        );

        let response = self.client
            .post(endpoint)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", "audio-24khz-48kbitrate-mono-mp3")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .body(ssml)
            .send()
            .await
            .map_err(|e| format!("Edge TTS 请求失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Edge TTS HTTP {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Edge TTS 读取响应失败: {}", e))?;

        Ok(bytes.to_vec())
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}
