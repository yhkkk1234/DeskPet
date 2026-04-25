use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityDimensions {
    pub openness: f64,
    pub conscientiousness: f64,
    pub extraversion: f64,
    pub agreeableness: f64,
    pub neuroticism: f64,
    pub creativity: f64,
}

impl PersonalityDimensions {
    pub fn generate(rng: &mut impl Rng) -> Self {
        Self {
            openness: sample_beta(rng, 2.0, 5.0),
            conscientiousness: sample_beta(rng, 2.0, 5.0),
            extraversion: sample_beta(rng, 2.0, 5.0),
            agreeableness: sample_beta(rng, 2.0, 5.0),
            neuroticism: sample_beta(rng, 2.0, 5.0),
            creativity: sample_beta(rng, 2.0, 5.0),
        }
    }

    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.openness,
            self.conscientiousness,
            self.extraversion,
            self.agreeableness,
            self.neuroticism,
            self.creativity,
        ]
    }

    pub fn labels() -> Vec<&'static str> {
        vec![
            "openness",
            "conscientiousness",
            "extraversion",
            "agreeableness",
            "neuroticism",
            "creativity",
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnateTendency {
    pub self_dims: PersonalityDimensions,
    pub preferred_dims: PersonalityDimensions,
}

impl InnateTendency {
    pub fn generate(rng: &mut impl Rng) -> Self {
        Self {
            self_dims: PersonalityDimensions::generate(rng),
            preferred_dims: PersonalityDimensions::generate(rng),
        }
    }

    pub fn apply_transfer_perturbation<R: Rng>(&mut self, rng: &mut R) {
        macro_rules! perturb {
            ($v:expr, $rng:expr) => {{
                let delta: f64 = $rng.gen_range(-0.08_f64..0.08_f64).abs();
                if $rng.gen_bool(0.5) { ($v + delta).min(1.0) } else { ($v - delta).max(0.0) }
            }};
        }
        self.self_dims.openness = perturb!(self.self_dims.openness, rng);
        self.self_dims.conscientiousness = perturb!(self.self_dims.conscientiousness, rng);
        self.self_dims.extraversion = perturb!(self.self_dims.extraversion, rng);
        self.self_dims.agreeableness = perturb!(self.self_dims.agreeableness, rng);
        self.self_dims.neuroticism = perturb!(self.self_dims.neuroticism, rng);
        self.self_dims.creativity = perturb!(self.self_dims.creativity, rng);
    }
}

fn sample_beta(rng: &mut impl Rng, alpha: f64, beta: f64) -> f64 {
    use rand_distr::Beta;
    let dist = Beta::new(alpha, beta).unwrap();
    rng.sample(dist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_generate_personality() {
        let mut rng = thread_rng();
        let dims = PersonalityDimensions::generate(&mut rng);
        for val in dims.to_vec() {
            assert!((0.0..=1.0).contains(&val), "Value {} out of range", val);
        }
    }

    #[test]
    fn test_generate_innate_tendency() {
        let mut rng = thread_rng();
        let it = InnateTendency::generate(&mut rng);
        assert!((0.0..=1.0).contains(&it.self_dims.openness));
        assert!((0.0..=1.0).contains(&it.preferred_dims.openness));
    }

    #[test]
    fn test_transfer_perturbation() {
        let mut rng = thread_rng();
        let mut it = InnateTendency::generate(&mut rng);
        let orig_openness = it.self_dims.openness;
        it.apply_transfer_perturbation(&mut rng);
        let diff = (it.self_dims.openness - orig_openness).abs();
        assert!(diff <= 0.1, "Perturbation too large: {}", diff);
    }
}