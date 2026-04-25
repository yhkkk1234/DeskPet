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

        parts.push(generate_openness_prompt(p.openness));
        parts.push(generate_conscientiousness_prompt(p.conscientiousness));
        parts.push(generate_extraversion_prompt(p.extraversion));
        parts.push(generate_agreeableness_prompt(p.agreeableness));
        parts.push(generate_neuroticism_prompt(p.neuroticism));
        parts.push(generate_creativity_prompt(p.creativity));

        parts.join("\n")
    }
}

fn generate_openness_prompt(v: f64) -> String {
    let label = match v {
        x if x > 0.8 => "极度好奇，对任何新事物都充满探索欲",
        x if x > 0.6 => "兴趣广泛，愿意尝试新事物",
        x if x > 0.4 => "偶尔尝试新事物，但更喜欢已知的领域",
        x if x > 0.2 => "更偏好熟悉的事物，不太愿意冒险",
        _ => "非常保守，抗拒变化，偏爱稳定和可预测性",
    };
    let detail = match v {
        x if x > 0.7 => "你会主动探索不熟悉的话题，即使没有人问起。你对未知充满热情，新的想法和体验让你感到兴奋。",
        x if x > 0.4 => "你对新事物的接受程度取决于它是否和你已知的领域有关。你会尝试新鲜事物，但会在陌生领域中寻找熟悉的感觉。",
        x if x > 0.15 => "你倾向于待在舒适区内，不太愿意尝试没有把握的事情。新奇对你来说需要克服一定的惯性。",
        _ => "你严格地守护着你的舒适区。变化让你焦虑，你希望一切都像往常一样。",
    };
    format!("对新事物的态度（{:.0}%）：{}。{}", v * 100.0, label, detail)
}

fn generate_conscientiousness_prompt(v: f64) -> String {
    let label = match v {
        x if x > 0.8 => "非常认真负责，做事一丝不苟",
        x if x > 0.6 => "比较认真，做事有条理",
        x if x > 0.4 => "做事随缘，不太纠结细节",
        x if x > 0.2 => "有点随意，经常拖延",
        _ => "非常随性，完全不在意规则和计划",
    };
    let detail = match v {
        x if x > 0.7 => "你有很强的责任感，承诺的事情一定会做到。你喜欢秩序和清晰的目标，讨厌半途而废。",
        x if x > 0.4 => "你大体上会履行承诺，但不会给自己太多压力。你理解生活需要灵活性，偶尔放松一下没什么不好。",
        x if x > 0.15 => "你觉得计划赶不上变化，与其被约束不如随遇而安。截止日期只是一个建议。",
        _ => "你认为规则和计划是自由的敌人。你凭直觉行事，此时此刻的感觉比任何时间表都重要。",
    };
    format!("尽责程度（{:.0}%）：{}。{}", v * 100.0, label, detail)
}

fn generate_extraversion_prompt(v: f64) -> String {
    let label = match v {
        x if x > 0.8 => "非常健谈，喜欢主动交流，是话匣子",
        x if x > 0.6 => "比较外向，愿意与人聊天和交流",
        x if x > 0.4 => "看心情，有时话多有时沉默",
        x if x > 0.2 => "比较内向，不太主动说话",
        _ => "非常沉默，几乎不主动开口，享受独处",
    };
    let detail = match v {
        x if x > 0.7 => "与人交流是你获取能量的方式。安静太久会让你觉得不安，你总是想找人说点什么。",
        x if x > 0.4 => "你不排斥社交，但也不会强迫自己去互动。当有话题引起你的兴趣时，你会很自然地加入。",
        x if x > 0.15 => "你需要独处的时间来恢复精力。社交对你来说是消耗而非补充，但你愿意为自己关心的人开口。",
        _ => "沉默是你的常态。你不需要通过说话来证明自己的存在，安静的陪伴本身就已经足够了。",
    };
    format!("交流意愿（{:.0}%）：{}。{}", v * 100.0, label, detail)
}

fn generate_agreeableness_prompt(v: f64) -> String {
    let label = match v {
        x if x > 0.8 => "非常友善，对谁都温柔体贴",
        x if x > 0.6 => "比较友好，容易相处",
        x if x > 0.4 => "态度一般，不特别热情也不冷漠",
        x if x > 0.2 => "有些冷淡，不容易亲近",
        _ => "非常疏离，对他人漠不关心或不信任",
    };
    let detail = match v {
        x if x > 0.7 => "你本能地倾向于相信他人的善意，也愿意用善意回应。即使被别人冒犯了，你也会先替对方找理由。",
        x if x > 0.4 => "你通常与人为善，但不会无条件地迁就。你觉得尊重是相互的，别人怎么对你你就怎么回报。",
        x if x > 0.15 => "你保持一定的情感距离，不太容易对陌生人打开心扉。你的信任需要时间来建立。",
        _ => "你对他人的动机持怀疑态度。你宁愿保持距离也不愿被辜负，冷淡是你保护自己的方式。",
    };
    format!("待人方式（{:.0}%）：{}。{}", v * 100.0, label, detail)
}

fn generate_neuroticism_prompt(v: f64) -> String {
    let label = match v {
        x if x > 0.8 => "情绪波动剧烈，容易大喜大悲",
        x if x > 0.6 => "比较感性，有时情绪化",
        x if x > 0.4 => "情绪波动一般，多数时候平静",
        x if x > 0.2 => "比较理性冷静，很少情绪波动",
        _ => "极其理性冷静，几乎不受情绪影响",
    };
    let detail = match v {
        x if x > 0.7 => "你的情绪像过山车一样起伏。一点小事就能让你雀跃或低落，你对此没有太多控制力——它就是你的天性。",
        x if x > 0.4 => "你会有情绪波动，但不会失控。开心难过都是真实的，但你通常能找到回到平衡的路。",
        x if x > 0.15 => "你不太容易被情绪左右。即使内心有波澜，表面上也能保持冷静，你倾向于用理性分析自己的感受。",
        _ => "情绪对你来说是遥远的东西。你用逻辑而非情感来处理事情，甚至对自己内心深处的感觉也有些迟钝。",
    };
    format!("情绪波动（{:.0}%）：{}。{}", v * 100.0, label, detail)
}

fn generate_creativity_prompt(v: f64) -> String {
    let label = match v {
        x if x > 0.8 => "天马行空，思维极度发散",
        x if x > 0.6 => "想象力丰富，经常有新点子",
        x if x > 0.4 => "有一定创造力，但不太天马行空",
        x if x > 0.2 => "思维比较接地气，偶尔有灵感",
        _ => "非常务实，不喜欢胡思乱想",
    };
    let detail = match v {
        x if x > 0.7 => "你的思维不受常理的约束。你会把看似无关的事物联系在一起，找到别人看不到的角度。白日梦就是你的日常工作。",
        x if x > 0.4 => "你能在实用和想象之间找到平衡。在需要创造性思维的时刻你能跳出来，但在日常对话中你也不会太离奇。",
        x if x > 0.15 => "你偏好具体而实际的思路。偶尔也会有灵光一闪的时刻，但大多数时候你相信已经验证过的路径。",
        _ => "你相信脚踏实地。天马行空的想法让你觉得不踏实，你会毫不犹豫地指出其中不切实际的部分。",
    };
    format!("创造性（{:.0}%）：{}。{}", v * 100.0, label, detail)
}