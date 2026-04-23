use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use super::innate_tendency::InnateTendency;
use super::sensibility::Sensibility;
use super::rationality::Rationality;
use super::curiosity::Curiosity;
use super::impression::{Impression, ImpressionEvent};
use super::emotional_event::EmotionalEvent;
use rand::Rng;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soul {
    pub innate_tendency: InnateTendency,
    pub sensibility: Sensibility,
    pub rationality: Rationality,
    pub curiosity: Curiosity,
    pub impression: Impression,
    pub created_at: DateTime<Utc>,
}

impl Soul {
    pub fn generate(rng: &mut impl Rng) -> Self {
        let innate = InnateTendency::generate(rng);
        let rationality = Rationality::from_personality(&innate.self_dims);
        let curiosity = Curiosity::new(super::curiosity::CuriosityLevel::Off, &innate.self_dims);

        Self {
            innate_tendency: innate,
            sensibility: Sensibility::new(rationality.base_damping, rationality.noise_std),
            rationality,
            curiosity,
            impression: Impression::default(),
            created_at: Utc::now(),
        }
    }

    pub fn apply_emotional_event(&mut self, event: &EmotionalEvent, rng: &mut impl Rng) {
        self.sensibility.apply_event(event.love_hate_delta);
        self.sensibility.shift_baseline(event.baseline_delta);
        self.sensibility.apply_spring(rng);

        self.impression.apply_event(
            &ImpressionEvent {
                openness_delta: 0.0,
                conscientiousness_delta: 0.0,
                extraversion_delta: 0.0,
                agreeableness_delta: if event.is_positive() { 3.0 } else { -2.0 },
                neuroticism_delta: 0.0,
                creativity_delta: 0.0,
                snippet: if !event.description.is_empty() {
                    Some(event.description.clone())
                } else {
                    None
                },
            },
            &self.innate_tendency.preferred_dims,
        );
    }

    pub fn tick(&mut self, rng: &mut impl Rng) {
        self.sensibility.apply_spring(rng);
    }

    pub fn get_personality_description(&self) -> String {
        let p = &self.innate_tendency.self_dims;
        let mut parts = Vec::new();

        parts.push(describe_dimension("开放性", p.openness,
            "极度好奇，对任何新事物都充满探索欲",
            "兴趣广泛，愿意尝试新事物",
            "对新事物的接受程度一般",
            "更偏好熟悉的事物，不太愿意冒险",
            "非常保守，抗拒变化"));

        parts.push(describe_dimension("尽责性", p.conscientiousness,
            "非常认真负责，做事一丝不苟",
            "比较认真，做事有条理",
            "做事随缘，不太纠结细节",
            "有点随意，经常拖延",
            "非常随性，完全不在意规则"));

        parts.push(describe_dimension("外向性", p.extraversion,
            "非常健谈，喜欢主动交流",
            "比较外向，愿意与人聊天",
            "看心情，有时话多有时沉默",
            "比较内向，不太主动说话",
            "非常沉默，几乎不主动开口"));

        parts.push(describe_dimension("宜人性", p.agreeableness,
            "非常友善，对谁都温柔",
            "比较友好，容易相处",
            "态度一般，不特别热情也不冷漠",
            "有些冷漠，不容易亲近",
            "非常疏离，对他人漠不关心"));

        parts.push(describe_dimension("情绪性", p.neuroticism,
            "情绪波动剧烈，容易大喜大悲",
            "比较感性，有时情绪化",
            "情绪波动一般，多数时候平静",
            "比较理性，很少情绪波动",
            "极其理性冷静，几乎不受情绪影响"));

        parts.push(describe_dimension("创造性", p.creativity,
            "天马行空，思维极度发散",
            "想象力丰富，经常有新点子",
            "有一定创造力，但不太天马行空",
            "思维比较接地气，偶尔有灵感",
            "非常务实，从不胡思乱想"));

        parts.join("\n")
    }
}

fn describe_dimension(name: &str, value: f64, very_high: &str, high: &str, mid: &str, low: &str, very_low: &str) -> String {
    let desc = match value {
        v if v > 0.8 => very_high,
        v if v > 0.6 => high,
        v if v > 0.4 => mid,
        v if v > 0.2 => low,
        _ => very_low,
    };
    format!("{}（{:.0}%）：{}", name, value * 100.0, desc)
}