use crate::core::soul::emotional_event::EmotionalEventType;
use crate::core::soul::impression::ImpressionEvent;
use crate::services::ai_service::AIService;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SentimentResult {
    pub event_type: String,
    pub intensity: f64,
    pub impression: ImpressionEvent,
    pub love_hate_hint: f64,
}

pub struct SentimentAnalyzer;

impl SentimentAnalyzer {
    pub async fn analyze(
        ai_service: &AIService,
        user_message: &str,
        pet_response: &str,
        pet_name: &str,
    ) -> Result<SentimentResult, String> {
        let system_prompt = format!(
            "你是{}内部的情感分析系统。分析刚才的对话，提取主人表现出的性格特征和情感事件。\
             你必须严格按JSON格式回复，不要包含任何其他文字。\
             JSON格式如下：\n\
             {{\n\
               \"event_type\": \"事件类型之一\",\n\
               \"intensity\": 0.0到1.0之间的强度值,\n\
               \"impression\": {{\n\
                 \"openness_delta\": 数值,\n\
                 \"conscientiousness_delta\": 数值,\n\
                 \"extraversion_delta\": 数值,\n\
                 \"agreeableness_delta\": 数值,\n\
                 \"neuroticism_delta\": 数值,\n\
                 \"creativity_delta\": 数值,\n\
                 \"snippet\": \"一句话印象总结\"\n\
               }},\n\
               \"love_hate_hint\": -5到5之间的情感倾向值\n\
             }}\n\n\
             事件类型可选值：\n\
             - UserInitiatedChat: 用户主动发起聊天\n\
              - UserCaredAboutPet: 用户表达了对桌宠的关心\n\
              - UserPraisedPet: 用户夸奖了桌宠\n\
              - UserSharedPersonalStory: 用户分享了个人经历\n\
              - UserCelebratedTogether: 用户和桌宠一起庆祝了什么\n\
              - NormalChat: 普通闲聊\n\
              - UserGotAngry: 用户生气了\n\
              - UserDismissedPet: 用户敷衍了桌宠\n\n\
             维度delta规则：\n\
             - 每个delta范围 -3.0 到 +3.0\n\
             - 正值表示该维度表现高，负值表示表现低\n\
             - 例如：用户分享了创意想法 → creativity_delta: 1~2\n\
             - 用户态度恶劣 → agreeableness_delta: -2~-3\n\n\
             love_hate_hint 规则：\n\
             - 综合这段对话的情感倾向\n\
             - 正值表示对话让桌宠开心，负值表示不开心\n\
             - 普通聊天约 0~1，关心夸奖约 2~5，生气骂人约 -3~-5",
            pet_name
        );

        let user_prompt = format!(
            "分析以下对话：\n\n主人说：{}\n\n{}回复：{}\n\n请输出JSON分析结果。",
            user_message, pet_name, pet_response
        );

        let messages = vec![crate::services::ai_service::ChatMessage {
            role: "user".into(),
            content: user_prompt,
        }];

        let raw_response = ai_service.chat(&system_prompt, &messages).await?;

        parse_analysis_response(&raw_response)
    }
}

/// 解析 AI 返回的情感分析 JSON（纯函数，可测试）。
/// 容错：提取 {..} 子串、缺失字段回退默认值、数值越界 clamp。
pub fn parse_analysis_response(raw_response: &str) -> Result<SentimentResult, String> {
    let json_str = extract_json(raw_response);

    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("情感分析JSON解析失败: {} (原文: {})", e, truncate_str(raw_response, 200)))?;

    let event_type_str = parsed["event_type"].as_str().unwrap_or("NormalChat");
    let intensity = parsed["intensity"].as_f64().unwrap_or(0.3).clamp(0.0, 1.0);
    let love_hate_hint = parsed["love_hate_hint"].as_f64().unwrap_or(0.0).clamp(-5.0, 5.0);

    let imp = &parsed["impression"];
    let snippet = imp["snippet"].as_str().unwrap_or("").to_string();

    let impression = ImpressionEvent {
        openness_delta: imp["openness_delta"].as_f64().unwrap_or(0.0).clamp(-3.0, 3.0),
        conscientiousness_delta: imp["conscientiousness_delta"].as_f64().unwrap_or(0.0).clamp(-3.0, 3.0),
        extraversion_delta: imp["extraversion_delta"].as_f64().unwrap_or(0.0).clamp(-3.0, 3.0),
        agreeableness_delta: imp["agreeableness_delta"].as_f64().unwrap_or(0.0).clamp(-3.0, 3.0),
        neuroticism_delta: imp["neuroticism_delta"].as_f64().unwrap_or(0.0).clamp(-3.0, 3.0),
        creativity_delta: imp["creativity_delta"].as_f64().unwrap_or(0.0).clamp(-3.0, 3.0),
        snippet: if snippet.is_empty() { None } else { Some(snippet) },
    };

    Ok(SentimentResult {
        event_type: event_type_str.to_string(),
        intensity,
        impression,
        love_hate_hint,
    })
}

fn extract_json(text: &str) -> &str {
    let start = text.find('{');
    let end = text.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => &text[s..=e],
        _ => text,
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    }
}

pub fn parse_event_type(type_str: &str) -> EmotionalEventType {
    match type_str {
        "UserInitiatedChat" => EmotionalEventType::UserInitiatedChat,
        "UserCaredAboutPet" => EmotionalEventType::UserCaredAboutPet,
        "UserPraisedPet" => EmotionalEventType::UserPraisedPet,
        "UserSharedPersonalStory" => EmotionalEventType::UserSharedPersonalStory,
        "UserCelebratedTogether" => EmotionalEventType::UserCelebratedTogether,
        "FirstConversation" => EmotionalEventType::FirstConversation,
        "BirthdayCelebrated" => EmotionalEventType::BirthdayCelebrated,
        "UserIgnoredPet" => EmotionalEventType::UserIgnoredPet,
        "UserGotAngry" => EmotionalEventType::UserGotAngry,
        "UserDismissedPet" => EmotionalEventType::UserDismissedPet,
        "CuriosityTriggered" => EmotionalEventType::CuriosityTriggered,
        _ => EmotionalEventType::NormalChat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_json() {
        let raw = r#"{
            "event_type": "UserPraisedPet",
            "intensity": 0.8,
            "impression": {
                "openness_delta": 1.5,
                "conscientiousness_delta": 0.0,
                "extraversion_delta": 0.5,
                "agreeableness_delta": 2.0,
                "neuroticism_delta": -1.0,
                "creativity_delta": 1.0,
                "snippet": "主人今天夸了我"
            },
            "love_hate_hint": 3.5
        }"#;
        let r = parse_analysis_response(raw).unwrap();
        assert_eq!(r.event_type, "UserPraisedPet");
        assert_eq!(r.intensity, 0.8);
        assert_eq!(r.love_hate_hint, 3.5);
        assert_eq!(r.impression.openness_delta, 1.5);
        assert_eq!(r.impression.agreeableness_delta, 2.0);
        assert_eq!(r.impression.snippet.as_deref(), Some("主人今天夸了我"));
    }

    #[test]
    fn test_parse_with_markdown_fence() {
        // AI 偶尔会包 ```json 围栏
        let raw = "好的，这是分析结果：\n```json\n{\"event_type\":\"NormalChat\",\"intensity\":0.3,\"impression\":{},\"love_hate_hint\":0.5}\n```";
        let r = parse_analysis_response(raw).unwrap();
        assert_eq!(r.event_type, "NormalChat");
        assert_eq!(r.intensity, 0.3);
        assert_eq!(r.love_hate_hint, 0.5);
    }

    #[test]
    fn test_parse_missing_fields_use_defaults() {
        // 空 impression 或缺字段 → 全部回退默认
        let raw = r#"{"event_type":"UserGotAngry"}"#;
        let r = parse_analysis_response(raw).unwrap();
        assert_eq!(r.event_type, "UserGotAngry");
        assert_eq!(r.intensity, 0.3); // 默认
        assert_eq!(r.love_hate_hint, 0.0); // 默认
        assert_eq!(r.impression.openness_delta, 0.0);
        assert_eq!(r.impression.snippet, None);
    }

    #[test]
    fn test_parse_clamps_out_of_range() {
        let raw = r#"{
            "event_type": "NormalChat",
            "intensity": 5.0,
            "impression": {"openness_delta": 10.0, "agreeableness_delta": -10.0},
            "love_hate_hint": 99.0
        }"#;
        let r = parse_analysis_response(raw).unwrap();
        assert_eq!(r.intensity, 1.0);
        assert_eq!(r.love_hate_hint, 5.0);
        assert_eq!(r.impression.openness_delta, 3.0);
        assert_eq!(r.impression.agreeableness_delta, -3.0);
    }

    #[test]
    fn test_parse_invalid_json_errors() {
        assert!(parse_analysis_response("这不是JSON").is_err());
        assert!(parse_analysis_response("").is_err());
    }

    #[test]
    fn test_extract_json_from_noisy_text() {
        assert_eq!(extract_json("前置文字{\"a\":1}后置文字"), "{\"a\":1}");
        assert_eq!(extract_json("{\"a\":1}"), "{\"a\":1}");
        assert_eq!(extract_json("无花括号"), "无花括号");
    }

    #[test]
    fn test_parse_event_type_mapping() {
        use crate::core::soul::emotional_event::EmotionalEventType;
        assert_eq!(parse_event_type("UserPraisedPet"), EmotionalEventType::UserPraisedPet);
        assert_eq!(parse_event_type("UserGotAngry"), EmotionalEventType::UserGotAngry);
        assert_eq!(parse_event_type("FirstConversation"), EmotionalEventType::FirstConversation);
        assert_eq!(parse_event_type("CuriosityTriggered"), EmotionalEventType::CuriosityTriggered);
        // 未知类型回退 NormalChat
        assert_eq!(parse_event_type("SomeUnknownEvent"), EmotionalEventType::NormalChat);
        assert_eq!(parse_event_type(""), EmotionalEventType::NormalChat);
    }
}