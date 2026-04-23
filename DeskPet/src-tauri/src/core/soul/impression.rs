use serde::{Deserialize, Serialize};
use super::innate_tendency::PersonalityDimensions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Impression {
    pub openness_score: f64,
    pub conscientiousness_score: f64,
    pub extraversion_score: f64,
    pub agreeableness_score: f64,
    pub neuroticism_score: f64,
    pub creativity_score: f64,
    pub general_impression_snippets: Vec<String>,
    pub overall_affinity: f64,
}

impl Default for Impression {
    fn default() -> Self {
        Self {
            openness_score: 0.0,
            conscientiousness_score: 0.0,
            extraversion_score: 0.0,
            agreeableness_score: 0.0,
            neuroticism_score: 0.0,
            creativity_score: 0.0,
            general_impression_snippets: Vec::new(),
            overall_affinity: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpressionEvent {
    pub openness_delta: f64,
    pub conscientiousness_delta: f64,
    pub extraversion_delta: f64,
    pub agreeableness_delta: f64,
    pub neuroticism_delta: f64,
    pub creativity_delta: f64,
    pub snippet: Option<String>,
}

impl Impression {
    pub fn apply_event(&mut self, event: &ImpressionEvent, preferred: &PersonalityDimensions) {
        self.openness_score += event.openness_delta;
        self.conscientiousness_score += event.conscientiousness_delta;
        self.extraversion_score += event.extraversion_delta;
        self.agreeableness_score += event.agreeableness_delta;
        self.neuroticism_score += event.neuroticism_delta;
        self.creativity_score += event.creativity_delta;

        self.overall_affinity =
            self.openness_score * preferred.openness
            + self.conscientiousness_score * preferred.conscientiousness
            + self.extraversion_score * preferred.extraversion
            + self.agreeableness_score * preferred.agreeableness
            + self.neuroticism_score * preferred.neuroticism
            + self.creativity_score * preferred.creativity;

        self.update_snippet(&event.snippet);
    }

    fn update_snippet(&mut self, snippet: &Option<String>) {
        if let Some(s) = snippet {
            if !s.is_empty() {
                self.general_impression_snippets.push(s.clone());
                while self.general_impression_snippets.len() > 20 {
                    self.general_impression_snippets.remove(0);
                }
            }
        }
    }

    pub fn get_latest_snippet(&self) -> Option<&String> {
        self.general_impression_snippets.last()
    }

    pub fn get_impression_prompt(&self) -> String {
        let mut parts = Vec::new();

        let dims = [
            ("开放性", self.openness_score),
            ("尽责性", self.conscientiousness_score),
            ("外向性", self.extraversion_score),
            ("宜人性", self.agreeableness_score),
            ("神经质", self.neuroticism_score),
            ("创造性", self.creativity_score),
        ];

        for (name, score) in dims {
            let desc = match score {
                s if s > 20.0 => format!("主人非常{}高", name),
                s if s > 5.0 => format!("主人{}偏高", name),
                s if s > -5.0 => format!("主人{}中等", name),
                s if s > -20.0 => format!("主人{}偏低", name),
                _ => format!("主人非常{}低", name),
            };
            parts.push(desc);
        }

        let mut result = format!("你对主人的印象：{}", parts.join("，"));

        if let Some(snippet) = self.get_latest_snippet() {
            result.push_str(&format!("。最近：{}", snippet));
        }

        let affinity_desc = match self.overall_affinity {
            a if a > 30.0 => "总体来说，你觉得主人是一个很好的人。",
            a if a > 10.0 => "总体来说，你对主人印象还不错。",
            a if a > -10.0 => "总体来说，你对主人没有特别的倾向。",
            a if a > -30.0 => "总体来说，你对主人印象不太好。",
            _ => "总体来说，你觉得主人不太合你的意。",
        };
        result.push_str(&format!(" {}", affinity_desc));

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::soul::innate_tendency::InnateTendency;
    use rand::thread_rng;

    #[test]
    fn test_impression_apply_event() {
        let mut rng = thread_rng();
        let it = InnateTendency::generate(&mut rng);
        let mut impression = Impression::default();

        let event = ImpressionEvent {
            openness_delta: 3.0,
            conscientiousness_delta: 5.0,
            extraversion_delta: 0.0,
            agreeableness_delta: -2.0,
            neuroticism_delta: 0.0,
            creativity_delta: 1.0,
            snippet: Some("主人喜欢聊技术".into()),
        };

        impression.apply_event(&event, &it.preferred_dims);
        assert_eq!(impression.openness_score, 3.0);
        assert_eq!(impression.conscientiousness_score, 5.0);
        assert_eq!(impression.general_impression_snippets.len(), 1);
    }

    #[test]
    fn test_snippet_overflow() {
        let mut impression = Impression::default();
        let mut rng = thread_rng();
        let it = InnateTendency::generate(&mut rng);

        for i in 0..25 {
            let event = ImpressionEvent {
                openness_delta: 1.0,
                conscientiousness_delta: 0.0,
                extraversion_delta: 0.0,
                agreeableness_delta: 0.0,
                neuroticism_delta: 0.0,
                creativity_delta: 0.0,
                snippet: Some(format!("印象{}", i)),
            };
            impression.apply_event(&event, &it.preferred_dims);
        }
        assert_eq!(impression.general_impression_snippets.len(), 20);
    }
}