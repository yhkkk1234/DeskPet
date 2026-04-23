use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sensibility {
    pub love_hate: f64,
    pub baseline: f64,
    pub base_damping: f64,
    pub noise_std: f64,
}

impl Default for Sensibility {
    fn default() -> Self {
        Self {
            love_hate: 0.0,
            baseline: 0.0,
            base_damping: 0.005,
            noise_std: 0.02,
        }
    }
}

impl Sensibility {
    pub fn new(base_damping: f64, noise_std: f64) -> Self {
        Self {
            love_hate: 0.0,
            baseline: 0.0,
            base_damping,
            noise_std,
        }
    }

    pub fn apply_spring(&mut self, rng: &mut impl Rng) {
        let deviation = self.love_hate - self.baseline;
        let abs_deviation = deviation.abs();

        let deviation_modifier = 1.0 / (1.0 + abs_deviation / 20.0);
        let spring_force = -deviation * self.base_damping * deviation_modifier;

        self.love_hate += spring_force;

        let noise: f64 = rng.gen_range(-3.0..3.0) * self.noise_std / 3.0;
        self.love_hate += noise;

        self.love_hate = self.love_hate.clamp(-100.0, 100.0);
    }

    pub fn apply_event(&mut self, value: f64) {
        if self.love_hate == 0.0 {
            self.love_hate += value;
        } else {
            let current_abs = self.love_hate.abs();
            let direction_modifier = 1.0 + current_abs / 200.0 * 0.3;

            let actual_value = if value.signum() == self.love_hate.signum() {
                value * direction_modifier
            } else {
                value / direction_modifier
            };

            self.love_hate += actual_value;
        }

        self.love_hate = self.love_hate.clamp(-100.0, 100.0);
    }

    pub fn shift_baseline(&mut self, delta: f64) {
        self.baseline = (self.baseline + delta).clamp(-30.0, 30.0);
    }

    pub fn get_tone_instruction(&self) -> &'static str {
        match self.love_hate {
            v if v > 50.0 => "请用温柔、关怀、体贴的语气与用户交流。适当使用亲昵的表达，会主动表达关心。",
            v if v > 20.0 => "请用友好、轻松的语气与用户交流。愿意多聊几句。",
            v if v > -20.0 => "你保持中立、平和的态度。正常交流即可。",
            v if v > -50.0 => "你对这个用户态度较为冷淡，回应简短。",
            _ => "你对这个用户有些冷淡，保持距离感，语言简洁。",
        }
    }

    pub fn get_warmth_score(&self) -> f64 {
        (self.love_hate + 100.0) / 200.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_spring_pulls_toward_baseline() {
        let mut s = Sensibility::default();
        s.love_hate = 50.0;
        s.baseline = 0.0;
        let mut rng = thread_rng();
        let initial = s.love_hate;
        for _ in 0..100 {
            s.apply_spring(&mut rng);
        }
        assert!(s.love_hate < initial, "Should have moved toward baseline");
    }

    #[test]
    fn test_event_same_direction_amplified() {
        let mut s = Sensibility::default();
        s.love_hate = 30.0;
        s.apply_event(10.0);
        assert!(s.love_hate > 40.0, "Same direction should be amplified: got {}", s.love_hate);
    }

    #[test]
    fn test_event_opposite_direction_weakened() {
        let mut s = Sensibility::default();
        s.love_hate = 30.0;
        s.apply_event(-10.0);
        assert!(s.love_hate > 20.0, "Opposite direction should be weakened: got {}", s.love_hate);
    }

    #[test]
    fn test_baseline_shift() {
        let mut s = Sensibility::default();
        s.shift_baseline(15.0);
        assert_eq!(s.baseline, 15.0);
        s.shift_baseline(20.0);
        assert_eq!(s.baseline, 30.0); // clamped at 30
    }

    #[test]
    fn test_deep_love_slow_return() {
        let mut s = Sensibility::default();
        s.love_hate = 90.0;
        s.baseline = 0.0;
        let mut rng = thread_rng();
        let after_10_ticks = {
            let mut s = s.clone();
            for _ in 0..10 {
                s.apply_spring(&mut rng);
            }
            s.love_hate
        };
        let mut s2 = Sensibility::default();
        s2.love_hate = 20.0;
        s2.baseline = 0.0;
        let after_10_ticks_shallow = {
            for _ in 0..10 {
                s2.apply_spring(&mut rng);
            }
            s2.love_hate
        };
        let deep_ratio = (90.0 - after_10_ticks) / 90.0;
        let shallow_ratio = (20.0 - after_10_ticks_shallow) / 20.0;
        assert!(deep_ratio < shallow_ratio, "Deep love should return slower: deep_ratio={}, shallow_ratio={}", deep_ratio, shallow_ratio);
    }
}