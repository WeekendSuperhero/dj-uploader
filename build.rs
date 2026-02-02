use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use rand::Rng;
use std::fs;
use std::path::Path;

fn encrypt_string(plaintext: &str, key: &[u8; 32], nonce: &[u8; 12]) -> String {
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(nonce);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("encryption failure");

    hex::encode(ciphertext)
}

fn main() {
    // Compile Slint UI
    slint_build::compile("ui/main.slint").expect("Failed to compile Slint UI");

    // Rest of build script
    // Read config.json at build time
    let config_path = Path::new("config.json");

    if !config_path.exists() {
        eprintln!("ERROR: config.json not found!");
        eprintln!("Please create config.json with your API credentials.");
        eprintln!("See config.json.example for template.");
        std::process::exit(1);
    }

    let config_content = fs::read_to_string(config_path).expect("Failed to read config.json");

    let config: serde_json::Value =
        serde_json::from_str(&config_content).expect("Failed to parse config.json");

    // Extract Mixcloud credentials
    let mixcloud = config
        .get("mixcloud")
        .expect("Missing 'mixcloud' in config.json");

    let client_id = mixcloud
        .get("client_id")
        .and_then(|v| v.as_str())
        .expect("Missing 'client_id' in mixcloud config");

    let client_secret = mixcloud
        .get("client_secret")
        .and_then(|v| v.as_str())
        .expect("Missing 'client_secret' in mixcloud config");

    // Generate a random 256-bit AES key and 96-bit nonce for this build
    let mut rng = rand::rng();
    let key: [u8; 32] = rng.r#random();
    let nonce: [u8; 12] = rng.r#random();

    // Encrypt the credentials with AES-256-GCM
    let encrypted_mc_id = encrypt_string(client_id, &key, &nonce);
    let encrypted_mc_secret = encrypt_string(client_secret, &key, &nonce);

    // Set environment variables for compile-time inclusion
    println!("cargo:rustc-env=MIXCLOUD_CLIENT_ID={}", encrypted_mc_id);
    println!(
        "cargo:rustc-env=MIXCLOUD_CLIENT_SECRET={}",
        encrypted_mc_secret
    );

    // Extract SoundCloud credentials (optional, with defaults)
    let (sc_id, sc_secret) = if let Some(soundcloud) = config.get("soundcloud") {
        (
            soundcloud
                .get("client_id")
                .and_then(|v| v.as_str())
                .unwrap_or("SOUNDCLOUD_CLIENT_ID_PLACEHOLDER"),
            soundcloud
                .get("client_secret")
                .and_then(|v| v.as_str())
                .unwrap_or("SOUNDCLOUD_CLIENT_SECRET_PLACEHOLDER"),
        )
    } else {
        (
            "SOUNDCLOUD_CLIENT_ID_PLACEHOLDER",
            "SOUNDCLOUD_CLIENT_SECRET_PLACEHOLDER",
        )
    };

    // Encrypt SoundCloud credentials
    let encrypted_sc_id = encrypt_string(sc_id, &key, &nonce);
    let encrypted_sc_secret = encrypt_string(sc_secret, &key, &nonce);

    println!("cargo:rustc-env=SOUNDCLOUD_CLIENT_ID={}", encrypted_sc_id);
    println!(
        "cargo:rustc-env=SOUNDCLOUD_CLIENT_SECRET={}",
        encrypted_sc_secret
    );

    // Store the encryption key and nonce as hex strings
    let key_hex = hex::encode(key);
    let nonce_hex = hex::encode(nonce);
    println!("cargo:rustc-env=ENCRYPTION_KEY={}", key_hex);
    println!("cargo:rustc-env=ENCRYPTION_NONCE={}", nonce_hex);

    if sc_id != "SOUNDCLOUD_CLIENT_ID_PLACEHOLDER" {
        println!("cargo:warning=Building with SoundCloud credentials");
    }

    // Rebuild if config.json changes
    println!("cargo:rerun-if-changed=config.json");

    println!("cargo:warning=Building with AES-256-GCM encrypted credentials");
}
