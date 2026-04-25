use once_cell::sync::OnceCell;

pub struct EdgeTTS;

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

static CLIENT: OnceCell<edge_tts_rust::EdgeTtsClient> = OnceCell::new();

fn get_client() -> Result<&'static edge_tts_rust::EdgeTtsClient, String> {
    CLIENT.get_or_try_init(|| {
        edge_tts_rust::EdgeTtsClient::new()
            .map_err(|e| format!("Edge TTS 初始化失败: {}", e))
    })
}

impl EdgeTTS {
    pub fn new() -> Self {
        Self
    }

    pub async fn synthesize(
        &self,
        text: &str,
        voice: &str,
    ) -> Result<Vec<u8>, String> {
        let client = get_client()?;

        let options = edge_tts_rust::SpeakOptions {
            voice: voice.to_string(),
            ..Default::default()
        };

        let result = client.synthesize(text, options).await
            .map_err(|e| format!("Edge TTS 合成失败: {}", e))?;

        if result.audio.is_empty() {
            return Err("Edge TTS 未返回音频数据".to_string());
        }

        Ok(result.audio)
    }
}
