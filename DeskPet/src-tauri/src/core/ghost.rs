use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::soul::soul::Soul;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ghost {
    pub version: String,
    pub ghost_id: String,
    pub soul_signature: String,
    pub created_at: DateTime<Utc>,
    pub generation: u32,
    pub transfer_history: Vec<TransferRecord>,
    pub body: BodyConfig,
    pub soul: Soul,
    pub name: String,
    pub persona: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRecord {
    pub timestamp: DateTime<Utc>,
    pub generation: u32,
    pub perturbation_seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyConfig {
    pub body_type: String,
    pub asset_path: String,
    pub default_expression: String,
    pub movement_style: String,
}

impl Default for BodyConfig {
    fn default() -> Self {
        Self {
            body_type: "spritesheet".into(),
            asset_path: "./bodies/default/".into(),
            default_expression: "idle".into(),
            movement_style: "walk".into(),
        }
    }
}

impl Ghost {
    pub fn generate(rng: &mut impl rand::Rng, name: Option<String>) -> Self {
        let soul = Soul::generate(rng);
        let innate = &soul.innate_tendency;

        let movement_style = derive_movement_style(&innate.self_dims);

        Self {
            version: "2.0".into(),
            ghost_id: Uuid::new_v4().to_string(),
            soul_signature: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            generation: 0,
            transfer_history: Vec::new(),
            body: BodyConfig {
                movement_style,
                ..BodyConfig::default()
            },
            soul,
            name: name.unwrap_or_else(|| "未命名".into()),
            persona: None,
        }
    }

    pub fn transfer(&mut self, rng: &mut impl rand::Rng) -> Self {
        self.soul_signature = Uuid::new_v4().to_string();

        self.soul.innate_tendency.apply_transfer_perturbation(rng);

        self.generation += 1;
        let record = TransferRecord {
            timestamp: Utc::now(),
            generation: self.generation,
            perturbation_seed: rng.gen(),
        };
        self.transfer_history.push(record);

        let mut transferred = self.clone();

        transferred.soul.sensibility.love_hate *= 0.8;

        transferred
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let ghost = Self::from_json(&json)?;
        Ok(ghost)
    }
}

fn derive_movement_style(p: &crate::core::soul::innate_tendency::PersonalityDimensions) -> String {
    let scores = [
        ("bouncy", p.extraversion),
        ("slide", p.agreeableness),
        ("float", p.openness),
        ("walk", p.conscientiousness),
        ("teleport", p.creativity),
    ];

    scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(style, _)| style.to_string())
        .unwrap_or_else(|| "walk".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_ghost_generate() {
        let mut rng = thread_rng();
        let ghost = Ghost::generate(&mut rng, Some("小花".into()));
        assert_eq!(ghost.version, "2.0");
        assert_eq!(ghost.name, "小花");
        assert!(!ghost.ghost_id.is_empty());
        assert!(!ghost.soul_signature.is_empty());
    }

    #[test]
    fn test_ghost_json_roundtrip() {
        let mut rng = thread_rng();
        let ghost = Ghost::generate(&mut rng, Some("测试".into()));
        let json = ghost.to_json().unwrap();
        let deserialized = Ghost::from_json(&json).unwrap();
        assert_eq!(ghost.ghost_id, deserialized.ghost_id);
        assert_eq!(ghost.name, deserialized.name);
    }

    #[test]
    fn test_ghost_transfer() {
        let mut rng = thread_rng();
        let mut ghost = Ghost::generate(&mut rng, Some("小花".into()));
        let _original_openness = ghost.soul.innate_tendency.self_dims.openness;
        let original_sig = ghost.soul_signature.clone();

        let _transferred = ghost.transfer(&mut rng);

        assert_ne!(ghost.soul_signature, original_sig);
        assert_eq!(ghost.generation, 1);
    }

    #[test]
    fn test_movement_style() {
        let mut rng = thread_rng();
        let ghost = Ghost::generate(&mut rng, None);
        let valid_styles = ["bouncy", "slide", "float", "walk", "teleport"];
        assert!(valid_styles.contains(&ghost.body.movement_style.as_str()));
    }
}
