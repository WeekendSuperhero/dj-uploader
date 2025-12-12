use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

// AES-256-GCM encrypted compile-time credentials (read from config.json during build)
const ENCRYPTED_MIXCLOUD_CLIENT_ID: &str = env!("MIXCLOUD_CLIENT_ID");
const ENCRYPTED_MIXCLOUD_CLIENT_SECRET: &str = env!("MIXCLOUD_CLIENT_SECRET");

// AES-256-GCM encrypted SoundCloud credentials
const ENCRYPTED_SOUNDCLOUD_CLIENT_ID: &str = env!("SOUNDCLOUD_CLIENT_ID");
const ENCRYPTED_SOUNDCLOUD_CLIENT_SECRET: &str = env!("SOUNDCLOUD_CLIENT_SECRET");

// Encryption key and nonce
const ENCRYPTION_KEY: &str = env!("ENCRYPTION_KEY");
const ENCRYPTION_NONCE: &str = env!("ENCRYPTION_NONCE");

fn decrypt_string(ciphertext_hex: &str) -> String {
    // Parse the encryption key and nonce from hex
    let key_bytes = hex::decode(ENCRYPTION_KEY).expect("Invalid encryption key");
    let nonce_bytes = hex::decode(ENCRYPTION_NONCE).expect("Invalid encryption nonce");

    // Parse ciphertext from hex
    let ciphertext = hex::decode(ciphertext_hex).expect("Invalid ciphertext hex");

    // Create cipher
    let key: [u8; 32] = key_bytes.try_into().expect("Key must be 32 bytes");
    let cipher = Aes256Gcm::new(&key.into());
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .expect("Decryption failed");

    String::from_utf8(plaintext).expect("Invalid UTF-8 after decryption")
}

#[derive(Debug, Clone)]
pub struct MixcloudCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl MixcloudCredentials {
    pub fn new() -> Self {
        Self {
            client_id: decrypt_string(ENCRYPTED_MIXCLOUD_CLIENT_ID),
            client_secret: decrypt_string(ENCRYPTED_MIXCLOUD_CLIENT_SECRET),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SoundcloudCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl SoundcloudCredentials {
    pub fn new() -> Self {
        Self {
            client_id: decrypt_string(ENCRYPTED_SOUNDCLOUD_CLIENT_ID),
            client_secret: decrypt_string(ENCRYPTED_SOUNDCLOUD_CLIENT_SECRET),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Seconds until token expires (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i64>,
}

impl TokenInfo {
    pub fn new(
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
    ) -> Self {
        Self {
            access_token,
            refresh_token,
            created_at: Utc::now(),
            expires_in,
        }
    }

    /// Check if token is expired or will expire soon (within 5 minutes)
    pub fn is_expired(&self) -> bool {
        if let Some(expires_in) = self.expires_in {
            let expiry_time = self.created_at + Duration::seconds(expires_in);
            let now = Utc::now();
            let buffer = Duration::minutes(5);

            now >= (expiry_time - buffer)
        } else {
            // If no expiry info, assume it's still valid
            false
        }
    }

    pub fn time_until_expiry(&self) -> Option<Duration> {
        if let Some(expires_in) = self.expires_in {
            let expiry_time = self.created_at + Duration::seconds(expires_in);
            let now = Utc::now();
            let remaining = expiry_time - now;

            if remaining.num_seconds() > 0 {
                Some(remaining)
            } else {
                Some(Duration::zero())
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixcloud: Option<TokenInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soundcloud: Option<TokenInfo>,
}

impl TokenStorage {
    pub fn load() -> Result<Self> {
        let token_path = Self::token_path()?;

        if !token_path.exists() {
            return Ok(Self {
                mixcloud: None,
                soundcloud: None,
            });
        }

        let contents = fs::read_to_string(&token_path).context("Failed to read token file")?;

        let storage: TokenStorage =
            serde_json::from_str(&contents).context("Failed to parse token file")?;

        Ok(storage)
    }

    pub fn save(&self) -> Result<()> {
        let token_path = Self::token_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = token_path.parent() {
            fs::create_dir_all(parent).context("Failed to create token directory")?;
        }

        let contents = serde_json::to_string_pretty(self).context("Failed to serialize tokens")?;

        fs::write(&token_path, contents).context("Failed to write token file")?;

        Ok(())
    }

    pub fn token_path() -> Result<PathBuf> {
        // Use XDG_CONFIG_HOME if set, otherwise ~/.config
        let config_dir = if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config)
        } else {
            dirs::home_dir()
                .context("Failed to determine home directory")?
                .join(".config")
        };

        Ok(config_dir.join("dj-uploader").join("tokens.json"))
    }

    pub fn set_mixcloud_tokens(&mut self, token_info: TokenInfo) {
        self.mixcloud = Some(token_info);
    }

    pub fn get_mixcloud_token(&self) -> Result<&TokenInfo> {
        self.mixcloud
            .as_ref()
            .context("Not authorized with Mixcloud. Run 'dj-uploader auth mixcloud' first")
    }
}
