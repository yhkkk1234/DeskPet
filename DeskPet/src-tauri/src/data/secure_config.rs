use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::data::{app_data_dir, get_or_create_master_key};
use crate::services::ai_service::AIProviderConfig;

/// AI 配置加密文件魔数（与 ghost 文件 "GF" 区分）。
const CONFIG_FILE_VERSION: &[u8; 2] = b"AC";
const CONFIG_FILE_NAME: &str = "ai_config.enc";

/// AI 配置的加密存储（AES-256-GCM，密钥与 ghost 文件共用 master.key）。
///
/// 替代将 API Key 明文写入 localStorage 的旧方案：
/// 敏感配置落盘前加密，前端只保留掩码标记，明文仅存在于后端内存。
pub struct SecureConfigManager;

fn derive_config_key(master_key: &[u8; 32], salt: &[u8]) -> [u8; 32] {
    let hkdf = Hkdf::<Sha256>::new(Some(salt), master_key);
    let mut file_key = [0u8; 32];
    hkdf.expand(b"deskpet-secure-config-key", &mut file_key)
        .expect("HKDF expand should not fail with 32-byte output");
    file_key
}

impl SecureConfigManager {
    pub fn save_ai_config(config: &AIProviderConfig) -> Result<(), String> {
        let dir = app_data_dir().map_err(|e| e.to_string())?;
        let path = dir.join(CONFIG_FILE_NAME);
        Self::save_ai_config_at(config, &path)
    }

    pub fn load_ai_config() -> Result<Option<AIProviderConfig>, String> {
        let dir = app_data_dir().map_err(|e| e.to_string())?;
        let path = dir.join(CONFIG_FILE_NAME);
        Self::load_ai_config_at(&path)
    }

    fn save_ai_config_at(config: &AIProviderConfig, path: &std::path::Path) -> Result<(), String> {
        let master_key = get_or_create_master_key().map_err(|e| e.to_string())?;

        let json = serde_json::to_vec(config).map_err(|e| e.to_string())?;

        let mut salt = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut salt);

        let file_key = derive_config_key(&master_key, &salt);

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let cipher = Aes256Gcm::new_from_slice(&file_key).map_err(|e| e.to_string())?;

        let ciphertext = cipher
            .encrypt(nonce, json.as_ref())
            .map_err(|e| format!("加密失败: {e}"))?;

        let mut file_data = Vec::new();
        file_data.extend_from_slice(CONFIG_FILE_VERSION);
        file_data.extend_from_slice(&salt);
        file_data.extend_from_slice(&nonce_bytes);
        file_data.extend_from_slice(&ciphertext);

        std::fs::write(path, file_data).map_err(|e| format!("写入配置失败: {e}"))?;
        Ok(())
    }

    fn load_ai_config_at(path: &std::path::Path) -> Result<Option<AIProviderConfig>, String> {
        if !path.exists() {
            return Ok(None);
        }

        let master_key = get_or_create_master_key().map_err(|e| e.to_string())?;
        let file_data = std::fs::read(path).map_err(|e| format!("读取配置失败: {e}"))?;

        if file_data.len() < 2 + 32 + 12 {
            return Err("配置文件格式无效".into());
        }
        if &file_data[..2] != CONFIG_FILE_VERSION {
            return Err("配置文件格式无效或版本不受支持".into());
        }

        let salt = &file_data[2..34];
        let nonce_bytes = &file_data[34..46];
        let ciphertext = &file_data[46..];

        let file_key = derive_config_key(&master_key, salt);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&file_key).map_err(|e| e.to_string())?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("解密配置失败: {e}"))?;

        serde_json::from_slice(&plaintext).map_err(|e| format!("解析配置失败: {e}"))
    }

    pub fn has_ai_config() -> bool {
        matches!(Self::load_ai_config(), Ok(Some(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AIProviderConfig {
        AIProviderConfig {
            endpoint: "https://api.deepseek.com/v1".into(),
            api_key: "sk-test-secret-key".into(),
            model: "deepseek-chat".into(),
            vision_model: Some("deepseek-vl".into()),
            image_model: None,
            image_gen_endpoint: Some("https://example.com/v1".into()),
            image_gen_api_key: Some("sk-image-secret".into()),
            is_default: true,
            stt_endpoint: None,
            stt_api_key: None,
            stt_model: None,
        }
    }

    #[test]
    fn test_ai_config_roundtrip() {
        let dir = std::env::temp_dir().join("deskpet_test_secure_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ai_config.enc");

        let original = test_config();
        SecureConfigManager::save_ai_config_at(&original, &path).unwrap();
        let loaded = SecureConfigManager::load_ai_config_at(&path).unwrap().unwrap();

        assert_eq!(loaded.endpoint, original.endpoint);
        assert_eq!(loaded.api_key, original.api_key);
        assert_eq!(loaded.model, original.model);
        assert_eq!(loaded.vision_model, original.vision_model);
        assert_eq!(loaded.image_model, original.image_model);
        assert_eq!(loaded.image_gen_endpoint, original.image_gen_endpoint);
        assert_eq!(loaded.image_gen_api_key, original.image_gen_api_key);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_ai_config_missing_file_returns_none() {
        let dir = std::env::temp_dir().join("deskpet_test_secure_config_missing");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nonexistent.enc");
        assert!(SecureConfigManager::load_ai_config_at(&path).unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
