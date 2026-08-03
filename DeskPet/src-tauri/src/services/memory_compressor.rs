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

        let json_str = extract_json_array(&response);
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

        let json_str = extract_json(&response);
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

        let json_str = extract_json_array(&response);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| format!("记忆模糊化JSON解析失败: {}", e))?;

        let mut results = Vec::new();
        for item in &parsed {
            let index = item["index"].as_u64().unwrap_or(0) as usize;
            if let Some(blurred) = item["blurred"].as_str() {
                if index > 0 && index <= summaries.len() {
                    results.push((summaries[index - 1].0.clone(), blurred.to_string()));
                }
            }
        }
        Ok(results)
    }
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
