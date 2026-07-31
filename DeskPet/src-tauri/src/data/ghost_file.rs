use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::core::ghost::Ghost;
use crate::data::get_or_create_master_key;

const GHOST_FILE_VERSION: &[u8; 2] = b"GF";

#[derive(Debug, thiserror::Error)]
pub enum GhostFileError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
}

pub struct GhostFileManager;

fn derive_file_key(master_key: &[u8; 32], salt: &[u8]) -> [u8; 32] {
    let hkdf = Hkdf::<Sha256>::new(Some(salt), master_key);
    let mut file_key = [0u8; 32];
    hkdf.expand(b"deskpet-ghost-file-key", &mut file_key)
        .expect("HKDF expand should not fail with 32-byte output");
    file_key
}

impl GhostFileManager {
    pub fn save_encrypted(
        ghost: &Ghost,
        path: &std::path::Path,
        master_key: Option<&[u8]>,
    ) -> Result<(), GhostFileError> {
        let key = match master_key {
            Some(k) => {
                let mut mk = [0u8; 32];
                mk.copy_from_slice(&k[..32]);
                mk
            }
            None => get_or_create_master_key()?,
        };

        let json = ghost.to_json()?;

        let mut salt = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut salt);

        let file_key = derive_file_key(&key, &salt);

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let cipher = Aes256Gcm::new_from_slice(&file_key)
            .map_err(|e| GhostFileError::Encryption(e.to_string()))?;

        let ciphertext = cipher
            .encrypt(nonce, json.as_bytes())
            .map_err(|e| GhostFileError::Encryption(e.to_string()))?;

        let mut file_data = Vec::new();
        file_data.extend_from_slice(GHOST_FILE_VERSION);
        file_data.extend_from_slice(&salt);
        file_data.extend_from_slice(&nonce_bytes);
        file_data.extend_from_slice(&ciphertext);

        std::fs::write(path, file_data)?;
        Ok(())
    }

    pub fn load_encrypted(
        path: &std::path::Path,
        master_key: Option<&[u8]>,
    ) -> Result<Ghost, GhostFileError> {
        let key = match master_key {
            Some(k) => {
                let mut mk = [0u8; 32];
                mk.copy_from_slice(&k[..32]);
                mk
            }
            None => get_or_create_master_key()?,
        };

        let file_data = std::fs::read(path)?;

        if file_data.len() < 2 + 32 + 12 {
            return Err(GhostFileError::InvalidFormat(
                "File too short to be a valid ghost file".into(),
            ));
        }

        if &file_data[..2] != GHOST_FILE_VERSION {
            return Err(GhostFileError::InvalidFormat(
                "Not a ghost file or unsupported version".into(),
            ));
        }

        let salt = &file_data[2..34];
        let nonce_bytes = &file_data[34..46];
        let ciphertext = &file_data[46..];

        let file_key = derive_file_key(&key, salt);

        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&file_key)
            .map_err(|e| GhostFileError::Decryption(e.to_string()))?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| GhostFileError::Decryption(e.to_string()))?;

        let json =
            String::from_utf8(plaintext).map_err(|e| GhostFileError::Decryption(e.to_string()))?;

        let ghost = Ghost::from_json(&json)?;
        Ok(ghost)
    }

    pub fn save_encrypted_with_auto_key(
        ghost: &Ghost,
        path: &std::path::Path,
    ) -> Result<(), GhostFileError> {
        Self::save_encrypted(ghost, path, None)
    }

    pub fn load_encrypted_with_auto_key(path: &std::path::Path) -> Result<Ghost, GhostFileError> {
        Self::load_encrypted(path, None)
    }

    pub fn is_encrypted_file(path: &std::path::Path) -> bool {
        if let Ok(data) = std::fs::read(path) {
            data.len() >= 2 && &data[..2] == GHOST_FILE_VERSION
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_encrypted_save_load_roundtrip() {
        let mut rng = thread_rng();
        let ghost = Ghost::generate(&mut rng, Some("加密测试".into()));
        let dir = std::env::temp_dir().join("deskpet_test_encrypt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.ghost");

        let master_key: [u8; 32] = [42u8; 32];

        GhostFileManager::save_encrypted(&ghost, &path, Some(&master_key)).unwrap();
        let loaded = GhostFileManager::load_encrypted(&path, Some(&master_key)).unwrap();

        assert_eq!(ghost.ghost_id, loaded.ghost_id);
        assert_eq!(ghost.name, loaded.name);
        assert_eq!(ghost.generation, loaded.generation);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wrong_key_fails_to_decrypt() {
        let mut rng = thread_rng();
        let ghost = Ghost::generate(&mut rng, Some("密钥测试".into()));
        let dir = std::env::temp_dir().join("deskpet_test_wrongkey");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.ghost");

        let key1: [u8; 32] = [1u8; 32];
        let key2: [u8; 32] = [2u8; 32];

        GhostFileManager::save_encrypted(&ghost, &path, Some(&key1)).unwrap();

        let result = GhostFileManager::load_encrypted(&path, Some(&key2));
        assert!(result.is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_encrypted_file_binary_format() {
        let mut rng = thread_rng();
        let ghost = Ghost::generate(&mut rng, Some("格式测试".into()));
        let dir = std::env::temp_dir().join("deskpet_test_format");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.ghost");

        let master_key: [u8; 32] = [42u8; 32];

        GhostFileManager::save_encrypted(&ghost, &path, Some(&master_key)).unwrap();

        assert!(GhostFileManager::is_encrypted_file(&path));

        let plaintext_path = dir.join("plain.ghost");
        ghost.save_to_file(&plaintext_path).unwrap();
        assert!(!GhostFileManager::is_encrypted_file(&plaintext_path));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_auto_key_roundtrip() {
        let mut rng = thread_rng();
        let ghost = Ghost::generate(&mut rng, Some("自动密钥测试".into()));
        let dir = std::env::temp_dir().join("deskpet_test_autokey");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.ghost");

        GhostFileManager::save_encrypted_with_auto_key(&ghost, &path).unwrap();
        let loaded = GhostFileManager::load_encrypted_with_auto_key(&path).unwrap();

        assert_eq!(ghost.ghost_id, loaded.ghost_id);
        assert_eq!(ghost.name, loaded.name);

        std::fs::remove_dir_all(&dir).ok();
    }
}
