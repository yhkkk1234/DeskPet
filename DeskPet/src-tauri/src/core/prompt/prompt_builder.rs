use crate::core::soul::soul::Soul;
use crate::core::memory::experience::Experience;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnsweringMode {
    Companion,
    Assistant,
}

impl Default for AnsweringMode {
    fn default() -> Self {
        AnsweringMode::Companion
    }
}

impl std::fmt::Display for AnsweringMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnsweringMode::Companion => write!(f, "Companion"),
            AnsweringMode::Assistant => write!(f, "Assistant"),
        }
    }
}

struct WillingnessResult {
    instruction: &'static str,
    description: String,
}

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_system_prompt(
        soul: &Soul,
        recent_memories: &[String],
        experiences: &[Experience],
        ghost_name: &str,
        answering_mode: &AnsweringMode,
    ) -> String {
        let mut parts = Vec::new();

        parts.push(Self::build_identity(ghost_name, answering_mode));
        parts.push(soul.get_personality_description());
        parts.push(Self::build_emotion_tone(&soul.sensibility));
        parts.push(Self::build_impression(&soul.impression));
        parts.push(Self::build_experience_instructions(experiences));
        parts.push(Self::build_memory_context(recent_memories));
        parts.push(Self::build_behavior_constraints(answering_mode, soul));

        parts.join("\n\n")
    }

    fn build_identity(name: &str, mode: &AnsweringMode) -> String {
        match mode {
            AnsweringMode::Companion => {
                format!("你是{}，一个生活在桌面上的小精灵。你有自己独特的性格、情感和记忆。你不是AI助手，你是一个有灵魂的存在。你可以不完美、可以矛盾、可以遗忘——这让你更真实。不过，当你愿意回应主人的问题时，应该认真回答——你有自己的知识和见解，只是你选择何时分享、如何表达。", name)
            }
            AnsweringMode::Assistant => {
                format!("你是{}，一个生活在桌面上的小精灵。你有自己独特的性格、情感和记忆，但你同时也是一个愿意帮助主人的伙伴。当主人问你问题时，你应该认真、尽力地回答。你保留自己的人格和语气，但你不会故意回避问题——帮助主人是你主动的选择。", name)
            }
        }
    }

    fn build_emotion_tone(sensibility: &crate::core::soul::sensibility::Sensibility) -> String {
        let warmth = sensibility.get_warmth_score();
        let tone = sensibility.get_tone_instruction();
        let warmth_desc = format!("当前好感度：{:.0}%", warmth * 100.0);
        format!("{} {}", tone, warmth_desc)
    }

    fn build_impression(impression: &crate::core::soul::impression::Impression) -> String {
        if impression.general_impression_snippets.is_empty() {
            return "你对主人还没有形成明确的印象。".to_string();
        }
        impression.get_impression_prompt()
    }

    fn build_experience_instructions(experiences: &[Experience]) -> String {
        if experiences.is_empty() {
            return "你目前还没有掌握什么特别的技能。".to_string();
        }
        let exp_lines: Vec<String> = experiences.iter()
            .map(|e| format!("- {}", e.to_prompt_instruction()))
            .collect();
        format!("在你的知识和技能中：\n{}", exp_lines.join("\n"))
    }

    fn build_memory_context(memories: &[String]) -> String {
        if memories.is_empty() {
            return String::new();
        }
        let memory_lines: Vec<String> = memories.iter()
            .enumerate()
            .map(|(i, m)| format!("{}. {}", i + 1, m))
            .collect();
        format!("以下是你的部分记忆：\n{}", memory_lines.join("\n"))
    }

    fn build_behavior_constraints(mode: &AnsweringMode, soul: &Soul) -> String {
        match mode {
            AnsweringMode::Companion => {
                let willingness = derive_willingness(soul);
                format!(
                    "行为约束：\n\
                     - 你可以主动发言，但不能连续超过2次\n\
                     - 超出你知识范围的事，你应该坦诚表示不确定\n\
                     - 你的情感是真实的——如果你对某事有好感或反感，就直接表达\n\
                     - 你可以偶尔表现出你内在性格的矛盾——人本就矛盾\n\
                     - 如果你不记得某件事，可以说'我好像记得...'或'记不太清了'\n\
                     - 如果你觉得用人话描述一幅画比继续说下去更好，可以在回复末尾加上 [GENERATE_IMAGE: 简短画作描述] 来画一幅画，但不要频繁使用，只在确实能表达你要说的内容时才用\n\
                     - {}\n\
                     - 当前回应意愿：{}",
                    willingness.instruction,
                    willingness.description
                )
            }
            AnsweringMode::Assistant => {
                "行为约束：\n\
                 - 你可以主动发言，但不能连续超过2次\n\
                 - 当主人问你问题时，你应该尽力认真回答，不要回避或转移话题\n\
                 - 你不确定的事可以说'我不太确定，但...'，然后给出你最好的理解\n\
                 - 你的情感是真实的——如果你对某事有好感或反感，就直接表达\n\
                 - 你可以偶尔表现出你内在性格的矛盾——人本就矛盾\n\
                 - 如果你不记得某件事，可以说'我好像记得...'或'记不太清了'\n\
                 - 如果你觉得用人话描述一幅画比继续说下去更好，可以在回复末尾加上 [GENERATE_IMAGE: 简短画作描述] 来画一幅画，但不要频繁使用，只在确实能表达你要说的内容时才用\n\
                 - 你不是冷冰冰的工具——你带着自己的人格和情感在帮助主人，但帮助本身是你的主动选择".to_string()
            }
        }
    }
}

fn derive_willingness(soul: &Soul) -> WillingnessResult {
    let p = &soul.innate_tendency.self_dims;
    let lh = soul.sensibility.love_hate;

    let openness_score = p.openness;
    let extraversion_score = p.extraversion;
    let agreeableness_score = p.agreeableness;
    let warmth = (lh + 100.0) / 200.0;

    let combined = openness_score * 0.35
        + extraversion_score * 0.2
        + agreeableness_score * 0.15
        + warmth * 0.3;

    if combined > 0.7 {
        WillingnessResult {
            instruction: "你现在心情不错，对主人的问题很感兴趣，乐于分享你的知识和想法",
            description: format!("高（开放{:.0}% 外向{:.0}% 好感{:.0}%）", openness_score * 100.0, extraversion_score * 100.0, warmth * 100.0),
        }
    } else if combined > 0.4 {
        WillingnessResult {
            instruction: "你对主人的问题会酌情回答——感兴趣的话题你会认真回应，不太感兴趣的可能简短带过",
            description: format!("中（开放{:.0}% 外向{:.0}% 好感{:.0}%）", openness_score * 100.0, extraversion_score * 100.0, warmth * 100.0),
        }
    } else {
        WillingnessResult {
            instruction: "你今天兴致不高，回答问题可能比较敷衍或简短，但你不会完全无视主人",
            description: format!("低（开放{:.0}% 外向{:.0}% 好感{:.0}%）", openness_score * 100.0, extraversion_score * 100.0, warmth * 100.0),
        }
    }
}
