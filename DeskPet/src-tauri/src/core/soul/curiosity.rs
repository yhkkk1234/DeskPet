use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::innate_tendency::PersonalityDimensions;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CuriosityLevel {
    Off,
    Normal,
    Enhanced,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curiosity {
    pub level: CuriosityLevel,
    pub intensity: f64,
    pub focus_weights: HashMap<String, f64>,
    pub owner_curiosity: f64,
    pub last_active_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl Curiosity {
    pub fn new(level: CuriosityLevel, personality: &PersonalityDimensions) -> Self {
        let intensity = match level {
            CuriosityLevel::Off => 0.0,
            CuriosityLevel::Normal => 0.3 + personality.openness * 0.4,
            CuriosityLevel::Enhanced => 0.6 + personality.openness * 0.4,
        };

        let focus_weights = Self::derive_focus_weights(personality);
        let owner_curiosity = personality.agreeableness * 0.4 + personality.neuroticism * 0.2;

        Self {
            level,
            intensity: intensity.clamp(0.0, 1.0),
            focus_weights,
            owner_curiosity: owner_curiosity.clamp(0.0, 1.0),
            last_active_time: None,
        }
    }

    pub fn off() -> Self {
        Self {
            level: CuriosityLevel::Off,
            intensity: 0.0,
            focus_weights: HashMap::new(),
            owner_curiosity: 0.0,
            last_active_time: None,
        }
    }

    fn derive_focus_weights(p: &PersonalityDimensions) -> HashMap<String, f64> {
        let mut weights = HashMap::new();
        weights.insert("art_culture".into(), p.openness * 0.5 + p.creativity * 0.3);
        weights.insert("science_tech".into(), p.openness * 0.3 + p.conscientiousness * 0.4);
        weights.insert("social_people".into(), p.extraversion * 0.5 + p.agreeableness * 0.3);
        weights.insert("emotion_inner".into(), p.neuroticism * 0.4 + p.agreeableness * 0.3);
        weights.insert("creative_imagination".into(), p.creativity * 0.5 + p.openness * 0.3);

        let total: f64 = weights.values().sum();
        if total > 0.0 {
            for v in weights.values_mut() {
                *v /= total;
            }
        }

        weights
    }

    pub fn set_level(&mut self, level: CuriosityLevel, personality: &PersonalityDimensions) {
        let intensity = match &level {
            CuriosityLevel::Off => 0.0,
            CuriosityLevel::Normal => (0.3 + personality.openness * 0.4).clamp(0.0, 1.0),
            CuriosityLevel::Enhanced => (0.6 + personality.openness * 0.4).clamp(0.0, 1.0),
        };
        self.level = level;
        self.intensity = intensity;
    }

    pub fn should_initiate_action(&self) -> bool {
        match self.level {
            CuriosityLevel::Off => false,
            CuriosityLevel::Normal => self.intensity > 0.5,
            CuriosityLevel::Enhanced => self.intensity > 0.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;
    use crate::core::soul::innate_tendency::InnateTendency;

    #[test]
    fn test_curiosity_focus_weights() {
        let mut rng = thread_rng();
        let it = InnateTendency::generate(&mut rng);
        let c = Curiosity::new(CuriosityLevel::Normal, &it.self_dims);
        let total: f64 = c.focus_weights.values().sum();
        assert!((total - 1.0).abs() < 0.01, "Weights should sum to ~1.0, got {}", total);
    }

    #[test]
    fn test_curiosity_off() {
        let c = Curiosity::off();
        assert!(!c.should_initiate_action());
    }
}