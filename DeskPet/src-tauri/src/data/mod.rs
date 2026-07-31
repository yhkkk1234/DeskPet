pub mod database;
pub mod ghost_file;
pub mod secure_config;

pub use ghost_file::{GhostFileError, GhostFileManager};

use std::path::PathBuf;

const KEY_FILE_NAME: &str = "master.key";

/// 获取 DeskPet 数据目录（%APPDATA%/DeskPet），不存在则创建。
pub fn app_data_dir() -> Result<PathBuf, std::io::Error> {
    let dir = dirs::data_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot determine app data directory",
        )
    })?;
    let app_dir = dir.join("DeskPet");
    std::fs::create_dir_all(&app_dir)?;
    Ok(app_dir)
}

/// 获取或创建全局加密主密钥（32 字节，存于 master.key）。
/// ghost 文件和加密配置共用同一把密钥，同一台机器可互相解密。
pub fn get_or_create_master_key() -> Result<[u8; 32], std::io::Error> {
    let app_dir = app_data_dir()?;
    let key_path = app_dir.join(KEY_FILE_NAME);

    for _ in 0..3 {
        if key_path.exists() {
            let key_bytes = std::fs::read(&key_path)?;
            if key_bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes);
                return Ok(key);
            }
            let _ = std::fs::remove_file(&key_path);
        }

        let mut key = [0u8; 32];
        use rand::RngCore;
        aes_gcm::aead::OsRng.fill_bytes(&mut key);
        std::fs::write(&key_path, key)?;
        return Ok(key);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Failed to create valid master key after 3 attempts",
    ))
}
