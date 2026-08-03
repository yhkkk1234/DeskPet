use crate::data::database::Database;
use crate::services::ai_service::{AIService, ChatMessage};

const STM_COMPRESS_THRESHOLD: usize = 20;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressResult {
    pub compressed: bool,
    pub new_ltm_count: usize,
    pub new_experience_count: usize,
}

pub struct MemoryCompressor;

#[derive(Debug, Clone)]
pub struct ParsedLTM {
    pub summary: String,
    pub importance: f64,
    pub is_core: bool,
    pub event_type: Option<String>,
}

#[derive(Debug)]
pub struct ExtractedExperience {
    pub name: String,
    pub summary: String,
    pub source: String,
    pub proficiency: f64,
}

impl MemoryCompressor {
    pub fn should_compress(db: &Database, ghost_id: &str) -> Result<bool, String> {
        let count = db.count_short_term_memories(ghost_id)?;
        Ok(count >= STM_COMPRESS_THRESHOLD)
    }

    pub fn fetch_stm_data(db: &Database, ghost_id: &str) -> Result<Vec<(String, f64, String)>, String> {
        let entries = db.get_short_term_memories(ghost_id, STM_COMPRESS_THRESHOLD)?;
        Ok(entries.iter().map(|m| (m.id.clone(), m.importance, m.summary.clone())).collect())
    }

    pub async fn compress_via_ai(
        ai_service: &AIService,
        stm_data: &[(String, f64, String)],
        ghost_name: &str,
    ) -> Result<Vec<ParsedLTM>, String> {
        let _ = ghost_name;

        let summaries: Vec<String> = stm_data.iter().enumerate().map(|(i, (_, imp, s))| {
            format!("{}. [重要性:{:.1}] {}", i + 1, imp, s)
        }).collect();

        let prompt = format!(
            "请将以下{}条短期记忆总结为1-3条长期记忆。每条记忆包含：\n\
             - 摘要：简洁概括核心内容\n\
             - 重要性：0.0到1.0\n\
             - 是否核心记忆（里程碑/第一次/重大转变）\n\
             - 事件类型（可选）\n\n\
             严格按JSON数组回复，不要其他文字：\n\
             [{{\"summary\": \"...\", \"importance\": 0.0, \"is_core\": false, \"event_type\": null}}]\n\n\
             短期记忆：\n{}",
            stm_data.len(),
            summaries.join("\n")
        );

        let messages = vec![ChatMessage { role: "user".into(), content: prompt }];
        let system_prompt = "你是记忆压缩系统，将多条短期记忆提炼为少量长期记忆。只输出JSON。";
        let response = ai_service.chat(system_prompt, &messages).await?;

        parse_ltm_response(&response)
    }

    pub async fn extract_experience_via_ai(
        ai_service: &AIService,
        summary: &str,
    ) -> Result<ExtractedExperience, String> {
        let prompt = format!(
            "以下是一段记忆：\n{}\n\n\
             是否包含可迁移知识或技能？如有用JSON回复：{{\"found\": true, \"name\": \"名称\", \"summary\": \"描述\", \"source\": \"来源\", \"proficiency\": 0.3}}\n\
             如无：{{\"found\": false}}",
            summary
        );
        let messages = vec![ChatMessage { role: "user".into(), content: prompt }];
        let system_prompt = "你是经验提取系统。从记忆中识别可迁移知识。只输出JSON。";
        let response = ai_service.chat(system_prompt, &messages).await?;

        parse_experience_response(&response)
    }

    pub fn save_compress_results(
        db: &Database,
        ghost_id: &str,
        ltm_items: &[ParsedLTM],
        stm_ids: &[String],
        experiences: &[ExtractedExperience],
    ) -> Result<CompressResult, String> {
        let mut new_ltm_count = 0;
        let mut new_experience_count = 0;
        let mut ltm_ids: Vec<String> = Vec::new();

        for item in ltm_items {
            let ltm_id = uuid::Uuid::new_v4().to_string();
            db.save_long_term_memory(
                &ltm_id, ghost_id, &item.summary, item.importance, item.is_core,
                item.event_type.as_deref(), None,
            )?;
            ltm_ids.push(ltm_id);
            new_ltm_count += 1;
        }

        for (i, exp) in experiences.iter().enumerate() {
            let source_id = ltm_ids.get(i).map(|s| s.as_str());
            db.save_experience(
                &uuid::Uuid::new_v4().to_string(), ghost_id,
                &exp.name, &exp.summary, &exp.source, exp.proficiency, source_id,
            )?;
            new_experience_count += 1;
        }

        for id in stm_ids {
            if let Err(e) = db.delete_short_term_memory(id) {
                tracing::error!("[记忆压缩] 删除短期记忆失败 (id={}): {}", id, e);
            }
        }

        Ok(CompressResult { compressed: true, new_ltm_count, new_experience_count })
    }

    /// AI 辅助记忆模糊化：将精确的记忆摘要改写为带有不确定感和模糊感的版本
    /// 模拟"原体消亡后记忆逐渐模糊"的叙事体验 (设计文档 §4.3)
    pub async fn blur_memory_summaries_via_ai(
        ai_service: &AIService,
        summaries: &[(String, String)],
        generation: u32,
    ) -> Result<Vec<(String, String)>, String> {
        if summaries.is_empty() {
            return Ok(Vec::new());
        }

        let blur_level = match generation {
            1 => "轻微模糊——将某些具体细节改为不太确定",
            2 => "中度模糊——将大部分细节模糊化，加入'好像''似乎''大概是'这类词",
            3 => "明显模糊——将具体事实变成模糊的印象，加入'记不太清了'的感觉",
            _ => "严重模糊——事实已经模糊不清，只剩下模糊的感觉和情绪基调",
        };

        let summaries_text: Vec<String> = summaries
            .iter()
            .enumerate()
            .map(|(i, (_, s))| format!("{}. {}", i + 1, s))
            .collect();

        let prompt = format!(
            "你需要模拟一个生命体经历第{}次灵魂传送后，核心记忆逐渐模糊化的过程。\n\
             模糊程度要求：{}\n\
             \n\
             以下是需要处理的记忆摘要列表，请为每一条生成模糊化后的版本。\n\
             - 保持记忆的核心事件，但让细节变得模糊\n\
             - 保留情感基调（开心/伤心/感动等），但弱化具体时间、地点、数字\n\
             - 每条只需输出一句话\n\
             \n\
             记忆列表：\n{}\n\
             \n\
             严格按JSON数组回复，不要其他文字：\n\
             [{{\"index\": 1, \"blurred\": \"模糊化后的文本\"}}, ...]",
            generation,
            blur_level,
            summaries_text.join("\n")
        );

        let messages = vec![ChatMessage { role: "user".into(), content: prompt }];
        let system_prompt = "你是记忆模糊化系统。你将精确的记忆改写为带有不确定感的模糊版本。只输出JSON。";
        let response = ai_service.chat(system_prompt, &messages).await?;

        Ok(parse_blur_response(&response, summaries))
    }
}

/// 解析 LTM 压缩响应（纯函数）：提取 [..] 数组，缺失字段回退，空摘要跳过。
pub fn parse_ltm_response(response: &str) -> Result<Vec<ParsedLTM>, String> {
    let json_str = extract_json_array(response);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str)
        .map_err(|e| format!("记忆压缩JSON解析失败: {}", e))?;

    let mut results = Vec::new();
    for item in &parsed {
        let summary = item["summary"].as_str().unwrap_or("").to_string();
        if summary.is_empty() { continue; }
        results.push(ParsedLTM {
            importance: item["importance"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0),
            is_core: item["is_core"].as_bool().unwrap_or(false),
            event_type: item["event_type"].as_str().map(|s| s.to_string()),
            summary,
        });
    }
    Ok(results)
}

/// 解析经验提取响应（纯函数）：found=false 时返回 Err("No extractable experience")。
pub fn parse_experience_response(response: &str) -> Result<ExtractedExperience, String> {
    let json_str = extract_json(response);
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("经验提取JSON解析失败: {}", e))?;

    if parsed["found"].as_bool().unwrap_or(false) {
        Ok(ExtractedExperience {
            name: parsed["name"].as_str().unwrap_or("未命名技能").to_string(),
            summary: parsed["summary"].as_str().unwrap_or("").to_string(),
            source: parsed["source"].as_str().unwrap_or("主人教会").to_string(),
            proficiency: parsed["proficiency"].as_f64().unwrap_or(0.3).clamp(0.1, 1.0),
        })
    } else {
        Err("No extractable experience".into())
    }
}

/// 解析记忆模糊化响应（纯函数）：按 index 映射回原摘要 id，越界丢弃。
pub fn parse_blur_response(
    response: &str,
    summaries: &[(String, String)],
) -> Vec<(String, String)> {
    let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(extract_json_array(response))
    else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for item in &parsed {
        let index = item["index"].as_u64().unwrap_or(0) as usize;
        if let Some(blurred) = item["blurred"].as_str() {
            if index > 0 && index <= summaries.len() {
                results.push((summaries[index - 1].0.clone(), blurred.to_string()));
            }
        }
    }
    results
}

fn extract_json(text: &str) -> &str {
    let start = text.find('{');
    let end = text.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => &text[s..=e],
        _ => text,
    }
}

fn extract_json_array(text: &str) -> &str {
    let start = text.find('[');
    let end = text.rfind(']');
    match (start, end) {
        (Some(s), Some(e)) if e > s => &text[s..=e],
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::database::Database;

    fn test_db() -> Database {
        Database::new(":memory:").expect("内存数据库初始化失败")
    }

    #[test]
    fn test_parse_ltm_response_full() {
        let raw = r#"[{"summary":"第一次见面很开心","importance":0.9,"is_core":true,"event_type":"FirstConversation"}]"#;
        let items = parse_ltm_response(raw).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].summary, "第一次见面很开心");
        assert_eq!(items[0].importance, 0.9);
        assert!(items[0].is_core);
        assert_eq!(items[0].event_type.as_deref(), Some("FirstConversation"));
    }

    #[test]
    fn test_parse_ltm_response_with_fence_and_defaults() {
        // markdown 围栏 + 缺失字段回退 + 空摘要跳过
        let raw = "以下是压缩结果：\n```json\n[{\"summary\":\"A\",\"importance\":3.0},{\"summary\":\"\",\"importance\":0.9}]\n```";
        let items = parse_ltm_response(raw).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].summary, "A");
        assert_eq!(items[0].importance, 1.0); // clamp
        assert!(!items[0].is_core); // 默认 false
        assert_eq!(items[0].event_type, None);
    }

    #[test]
    fn test_parse_ltm_response_invalid() {
        assert!(parse_ltm_response("不是数组").is_err());
    }

    #[test]
    fn test_parse_experience_found() {
        let raw = r#"{"found":true,"name":"Rust所有权","summary":"理解了借用规则","source":"对话总结","proficiency":0.6}"#;
        let exp = parse_experience_response(raw).unwrap();
        assert_eq!(exp.name, "Rust所有权");
        assert_eq!(exp.source, "对话总结");
        assert_eq!(exp.proficiency, 0.6);
    }

    #[test]
    fn test_parse_experience_not_found() {
        assert!(parse_experience_response(r#"{"found":false}"#).is_err());
        // 缺失 found 字段也视为未找到
        assert!(parse_experience_response("{}").is_err());
    }

    #[test]
    fn test_parse_blur_response_mapping() {
        let summaries = vec![("id1".to_string(), "原始记忆1".to_string()), ("id2".to_string(), "原始记忆2".to_string())];
        let raw = r#"[{"index":1,"blurred":"好像发生过什么"},{"index":2,"blurred":"记不清了"},{"index":99,"blurred":"越界应丢弃"}]"#;
        let results = parse_blur_response(raw, &summaries);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "id1");
        assert_eq!(results[0].1, "好像发生过什么");
        assert_eq!(results[1].0, "id2");
        // 越界 index 丢弃
    }

    #[test]
    fn test_should_compress_threshold() {
        let db = test_db();
        assert!(!MemoryCompressor::should_compress(&db, "g1").unwrap());
        for i in 0..20 {
            db.save_short_term_memory(&format!("s{}", i), "g1", &format!("记忆{}", i), 0.5, 1.0, 0.0, None, None)
                .unwrap();
        }
        assert!(MemoryCompressor::should_compress(&db, "g1").unwrap());
        // 其他 ghost 不受影响
        assert!(!MemoryCompressor::should_compress(&db, "g2").unwrap());
    }

    #[test]
    fn test_fetch_stm_data_orders_by_importance() {
        let db = test_db();
        db.save_short_term_memory("low", "g1", "低重要性", 0.1, 1.0, 0.0, None, None).unwrap();
        db.save_short_term_memory("high", "g1", "高重要性", 0.9, 1.0, 0.0, None, None).unwrap();
        let data = MemoryCompressor::fetch_stm_data(&db, "g1").unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].0, "high"); // importance DESC
        assert_eq!(data[0].2, "高重要性");
    }

    #[test]
    fn test_save_compress_results_full_flow() {
        let db = test_db();
        // 造 3 条 STM
        for i in 0..3 {
            db.save_short_term_memory(&format!("s{}", i), "g1", &format!("短期记忆{}", i), 0.5, 1.0, 0.0, None, None)
                .unwrap();
        }
        let ltm = vec![
            ParsedLTM { summary: "长期记忆A".into(), importance: 0.8, is_core: true, event_type: Some("FirstConversation".into()) },
        ];
        let exp = vec![
            ExtractedExperience { name: "技能X".into(), summary: "学会X".into(), source: "对话".into(), proficiency: 0.5 },
        ];
        let stm_ids = vec!["s0".to_string(), "s1".to_string(), "s2".to_string()];

        let result = MemoryCompressor::save_compress_results(&db, "g1", &ltm, &stm_ids, &exp).unwrap();
        assert_eq!(result.new_ltm_count, 1);
        assert_eq!(result.new_experience_count, 1);

        // LTM 已保存（含 core 标记）
        let ltms = db.get_long_term_memories("g1", 10).unwrap();
        assert_eq!(ltms.len(), 1);
        assert!(ltms[0].is_core_memory);

        // 经验已保存且关联到 LTM id
        let exps = db.get_experiences("g1").unwrap();
        assert_eq!(exps.len(), 1);
        assert_eq!(exps[0].name, "技能X");
        assert!(exps[0].source_memory_id.is_some());

        // STM 已清除
        assert_eq!(db.count_short_term_memories("g1").unwrap(), 0);
    }

    #[test]
    fn test_save_compress_results_clears_only_matching_stm() {
        let db = test_db();
        db.save_short_term_memory("keep", "g2", "别的灵魂的记忆", 0.5, 1.0, 0.0, None, None).unwrap();
        db.save_short_term_memory("del", "g1", "要删的", 0.5, 1.0, 0.0, None, None).unwrap();

        MemoryCompressor::save_compress_results(&db, "g1", &[], &["del".to_string()], &[]).unwrap();
        assert_eq!(db.count_short_term_memories("g1").unwrap(), 0);
        assert_eq!(db.count_short_term_memories("g2").unwrap(), 1);
    }
}
