use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// The number of PBKDF2 iterations for key derivation.
const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;

#[derive(serde::Serialize, serde::Deserialize)]
struct VaultFile {
    salt: String,
    nonce: String,
    cipher_text: String,
}

pub struct Vault;

impl Vault {
    /// Gets the path to the vault file, defaulting to `.env.vault` in current dir.
    fn get_vault_path() -> PathBuf {
        PathBuf::from(".env.vault")
    }

    /// Derives an AES-256 key from a master password and a salt using PBKDF2.
    fn derive_key(password: &str, salt: &[u8]) -> Key<Aes256Gcm> {
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
        Key::<Aes256Gcm>::from_slice(&key).to_owned()
    }

    /// Initializes a new, empty vault protected by the given password.
    pub fn init(password: &str) -> Result<()> {
        let path = Self::get_vault_path();
        if path.exists() {
            return Err(anyhow!("Vault already exists at {:?}", path));
        }

        let empty_env: HashMap<String, String> = HashMap::new();
        Self::save_vault(password, &empty_env)?;
        Ok(())
    }

    /// Sets or updates a key-value pair in the vault.
    pub fn set(password: &str, key: &str, value: &str) -> Result<()> {
        let mut env_map = Self::load_vault(password).unwrap_or_default();
        env_map.insert(key.to_string(), value.to_string());
        Self::save_vault(password, &env_map)
    }

    /// Unseals the vault using the master password and loads all keys into the environment.
    pub fn unseal(password: &str) -> Result<()> {
        let env_map = Self::load_vault(password)?;
        for (k, v) in env_map {
            env::set_var(k, v);
        }
        Ok(())
    }

    /// Reads and decrypts the vault into a HashMap.
    fn load_vault(password: &str) -> Result<HashMap<String, String>> {
        let path = Self::get_vault_path();
        if !path.exists() {
            return Err(anyhow!("Vault not found. Please run 'cortex vault init'"));
        }

        let file_contents = fs::read_to_string(path)?;
        let vault_file: VaultFile = serde_json::from_str(&file_contents)?;

        let salt = hex::decode(vault_file.salt)?;
        let nonce_bytes = hex::decode(vault_file.nonce)?;
        let cipher_text = hex::decode(vault_file.cipher_text)?;

        let key = Self::derive_key(password, &salt);
        let cipher = Aes256Gcm::new(&key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plain_text = cipher
            .decrypt(nonce, cipher_text.as_ref())
            .map_err(|_| anyhow!("Failed to decrypt vault. Incorrect master password?"))?;

        let env_map: HashMap<String, String> = serde_json::from_slice(&plain_text)?;
        Ok(env_map)
    }

    /// Encrypts and writes the HashMap to the vault.
    fn save_vault(password: &str, env_map: &HashMap<String, String>) -> Result<()> {
        let path = Self::get_vault_path();

        // 1. Generate salt
        use rand::Rng;
        let mut salt = [0u8; SALT_LEN];
        rand::rng().fill_bytes(&mut salt);

        // 2. Derive key
        let key = Self::derive_key(password, &salt);
        let cipher = Aes256Gcm::new(&key);

        // 3. Generate internal AES-GCM nonce using OsRng via generate_nonce directly
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // 4. Encrypt payload
        let payload = serde_json::to_vec(env_map)?;
        let cipher_text = cipher
            .encrypt(&nonce, payload.as_ref())
            .map_err(|e| anyhow!("Encryption error: {}", e))?;

        // 5. Save to disk
        let vault_file = VaultFile {
            salt: hex::encode(salt),
            nonce: hex::encode(nonce),
            cipher_text: hex::encode(cipher_text),
        };

        let json = serde_json::to_string_pretty(&vault_file)?;
        fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_encrypt_decrypt() -> Result<()> {
        let password = "test-master-password";
        let mut data = HashMap::new();
        data.insert("API_KEY".to_string(), "sk-12345".to_string());
        data.insert("DB_URL".to_string(), "postgres://localhost".to_string());

        // Test save
        Vault::save_vault(password, &data)?;

        // Test load
        let loaded = Vault::load_vault(password)?;
        assert_eq!(loaded.get("API_KEY").unwrap(), "sk-12345");
        assert_eq!(loaded.get("DB_URL").unwrap(), "postgres://localhost");

        // Cleanup
        let _ = fs::remove_file(Vault::get_vault_path());
        Ok(())
    }

    #[test]
    fn test_vault_wrong_password() -> Result<()> {
        let password = "correct-password";
        let mut data = HashMap::new();
        data.insert("SECRET".to_string(), "hidden".to_string());
        Vault::save_vault(password, &data)?;

        // Try wrong password
        let result = Vault::load_vault("wrong-password");
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_file(Vault::get_vault_path());
        Ok(())
    }
}

