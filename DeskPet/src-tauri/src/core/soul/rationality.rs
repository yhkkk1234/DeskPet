use super::innate_tendency::PersonalityDimensions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rationality {
    pub base_damping: f64,
    pub noise_std: f64,
}

impl Rationality {
    pub fn from_personality(personality: &PersonalityDimensions) -> Self {
        let conscientiousness = personality.conscientiousness;
        let base_damping = 0.002 + conscientiousness * 0.008;
        let noise_std = 0.03 - conscientiousness * 0.02;

        Self {
            base_damping: base_damping.clamp(0.001, 0.01),
            noise_std: noise_std.clamp(0.005, 0.03),
        }
    }

    pub fn default_values() -> Self {
        Self {
            base_damping: 0.005,
            noise_std: 0.02,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::soul::innate_tendency::PersonalityDimensions;

    #[test]
    fn test_high_conscientiousness_strong_rationality() {
        let p = PersonalityDimensions {
            openness: 0.5,
            conscientiousness: 0.9,
            extraversion: 0.5,
            agreeableness: 0.5,
            neuroticism: 0.5,
            creativity: 0.5,
        };
        let r = Rationality::from_personality(&p);
        assert!(r.base_damping > 0.007, "High conscientiousness should have strong damping");
        assert!(r.noise_std < 0.015, "High conscientiousness should have low noise");
    }

    #[test]
    fn test_low_conscientiousness_weak_rationality() {
        let p = PersonalityDimensions {
            openness: 0.5,
            conscientiousness: 0.1,
            extraversion: 0.5,
            agreeableness: 0.5,
            neuroticism: 0.5,
            creativity: 0.5,
        };
        let r = Rationality::from_personality(&p);
        assert!(r.base_damping < 0.004, "Low conscientiousness should have weak damping");
    }
}