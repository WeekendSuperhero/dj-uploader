use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use log::{debug, info, warn};
use rand::Rng;
use reqwest::blocking::{Client, multipart};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use url::Url;

use crate::config::{SoundcloudCredentials, TokenInfo, TokenStorage};

const OAUTH_AUTHORIZE_URL: &str = "https://secure.soundcloud.com/authorize";
const OAUTH_TOKEN_URL: &str = "https://secure.soundcloud.com/oauth/token";
const UPLOAD_URL: &str = "https://api.soundcloud.com/tracks";
const REDIRECT_URI: &str = "http://localhost:8889/callback";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadResponse {
    pub id: i64,
    #[serde(default)]
    pub permalink_url: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Generate PKCE code verifier (random string)
fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..32).map(|_| rng.r#gen()).collect();
    URL_SAFE_NO_PAD.encode(&random_bytes)
}

/// Generate PKCE code challenge from verifier (SHA256)
fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

pub struct SoundcloudClient {
    client: Client,
    credentials: SoundcloudCredentials,
    token_storage: TokenStorage,
}

impl SoundcloudClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .context("Failed to create HTTP client")?;

        let credentials = SoundcloudCredentials::new();
        let token_storage = TokenStorage::load()?;

        Ok(Self {
            client,
            credentials,
            token_storage,
        })
    }

    pub fn authorize() -> Result<()> {
        info!("Starting SoundCloud OAuth2 authorization with PKCE...");

        let credentials = SoundcloudCredentials::new();

        // Generate PKCE values
        let code_verifier = generate_code_verifier();
        let code_challenge = generate_code_challenge(&code_verifier);

        // Generate random state for CSRF protection
        let state: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        // Build authorization URL
        let mut auth_url = Url::parse(OAUTH_AUTHORIZE_URL)?;
        auth_url
            .query_pairs_mut()
            .append_pair("client_id", &credentials.client_id)
            .append_pair("redirect_uri", REDIRECT_URI)
            .append_pair("response_type", "code")
            .append_pair("code_challenge", &code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &state)
            .append_pair("scope", "non-expiring"); // Request non-expiring token

        println!("\nOpening browser for authorization...");
        println!("If the browser doesn't open, visit this URL:\n");
        println!("{}\n", auth_url);

        // Open browser
        if let Err(e) = webbrowser::open(auth_url.as_str()) {
            eprintln!("Failed to open browser: {}", e);
        }

        // Start local server to receive callback
        let listener = TcpListener::bind("127.0.0.1:8889")
            .context("Failed to start callback server. Is port 8889 already in use?")?;

        println!("Waiting for authorization...");

        let (mut stream, _) = listener.accept()?;
        let buf_reader = BufReader::new(&stream);
        let request_line = buf_reader
            .lines()
            .next()
            .context("Failed to read request")?
            .context("Empty request")?;

        // Parse the authorization code from the request
        let (code, returned_state) = Self::extract_code_from_request(&request_line)?;

        // Validate state
        if returned_state != state {
            bail!("State mismatch - possible CSRF attack");
        }

        // Send success response to browser
        let html = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Authorization Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #ff8800 0%, #ff3300 100%);
            color: white;
        }
        .container {
            text-align: center;
            padding: 2rem;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            backdrop-filter: blur(10px);
        }
        h1 { margin: 0 0 1rem 0; }
        p { margin: 0; opacity: 0.9; }
    </style>
</head>
<body>
    <div class="container">
        <h1>✓ Authorization Successful!</h1>
        <p>You can close this window and return to the terminal.</p>
        <p style="margin-top: 1rem; font-size: 0.9em;">This window will close automatically...</p>
    </div>
    <script>
        setTimeout(function() { window.close(); }, 2000);
    </script>
</body>
</html>
"#;

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            html.len(),
            html
        );
        stream.write_all(response.as_bytes())?;

        info!("Received authorization code, exchanging for access token...");

        // Exchange code for access token
        let http_client = Client::new();
        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code".to_string());
        params.insert("client_id", credentials.client_id.clone());
        params.insert("client_secret", credentials.client_secret.clone());
        params.insert("redirect_uri", REDIRECT_URI.to_string());
        params.insert("code", code);
        params.insert("code_verifier", code_verifier);

        let response = http_client
            .post(OAUTH_TOKEN_URL)
            .form(&params)
            .send()
            .context("Failed to exchange authorization code")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("Token exchange failed with status {}: {}", status, body);
        }

        let token_response: TokenResponse =
            response.json().context("Failed to parse token response")?;

        // Save tokens to storage
        let token_info = TokenInfo::new(
            token_response.access_token,
            token_response.refresh_token,
            token_response.expires_in,
        );

        let mut storage = TokenStorage::load().unwrap_or(TokenStorage {
            mixcloud: None,
            soundcloud: None,
        });
        storage.soundcloud = Some(token_info);
        storage.save()?;

        println!("\n✓ Authorization successful!");
        println!("Token saved to: {}", TokenStorage::token_path()?.display());

        if let Some(expires_in) = token_response.expires_in {
            let hours = expires_in / 3600;
            println!("Token expires in {} hours", hours);
        }

        println!("\nYou can now upload tracks with:");
        println!("  dj-uploader upload soundcloud --file <path> --title \"Your Track\"");

        Ok(())
    }

    fn extract_code_from_request(request_line: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            bail!("Invalid request format");
        }

        let path = parts[1];
        let url = Url::parse(&format!("http://localhost{}", path))?;

        let code = url
            .query_pairs()
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.to_string())
            .context("Authorization code not found in callback")?;

        let state = url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.to_string())
            .context("State not found in callback")?;

        Ok((code, state))
    }

    fn refresh_token_if_needed(&mut self) -> Result<()> {
        let token_info = self
            .token_storage
            .soundcloud
            .as_ref()
            .context("No SoundCloud token available")?;

        if token_info.is_expired() {
            warn!("Access token is expired or expiring soon, refreshing...");

            let refresh_token = token_info.refresh_token.as_ref().context(
                "No refresh token available. Please re-authorize with 'dj-uploader auth soundcloud'",
            )?;

            let mut params = HashMap::new();
            params.insert("grant_type", "refresh_token".to_string());
            params.insert("client_id", self.credentials.client_id.clone());
            params.insert("client_secret", self.credentials.client_secret.clone());
            params.insert("refresh_token", refresh_token.clone());

            let response = self
                .client
                .post(OAUTH_TOKEN_URL)
                .form(&params)
                .send()
                .context("Failed to refresh token")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                bail!(
                    "Token refresh failed with status {}: {}. Please re-authorize.",
                    status,
                    body
                );
            }

            let token_response: TokenResponse = response
                .json()
                .context("Failed to parse token refresh response")?;

            // Update token storage
            let new_token_info = TokenInfo::new(
                token_response.access_token,
                token_response.refresh_token.or(Some(refresh_token.clone())),
                token_response.expires_in,
            );

            self.token_storage.soundcloud = Some(new_token_info);
            self.token_storage.save()?;

            info!("Token refreshed successfully");
        }

        Ok(())
    }

    pub fn upload(
        &mut self,
        file_path: &Path,
        title: &str,
        description: Option<&str>,
        image_path: Option<&Path>,
        tags: Option<Vec<String>>,
    ) -> Result<UploadResponse> {
        // Check if we have a token, if not, authorize first
        if self.token_storage.soundcloud.is_none() {
            println!("\nNo authorization found. Starting OAuth2 flow...\n");
            Self::authorize()?;
            // Reload token storage after authorization
            self.token_storage = TokenStorage::load()?;
        }

        // Refresh token if needed
        self.refresh_token_if_needed()?;

        let token_info = self
            .token_storage
            .soundcloud
            .as_ref()
            .context("No SoundCloud token")?;

        info!("Uploading {} to SoundCloud...", file_path.display());

        if !file_path.exists() {
            bail!("File not found: {}", file_path.display());
        }

        // Build multipart form
        let mut form = multipart::Form::new();

        // Add audio file
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid file name")?
            .to_string();

        let file_bytes = fs::read(file_path).context("Failed to read audio file")?;

        let file_part = multipart::Part::bytes(file_bytes)
            .file_name(file_name.clone())
            .mime_str("audio/mpeg")?;

        form = form.part("track[asset_data]", file_part);

        // Add metadata
        form = form.text("track[title]", title.to_string());

        if let Some(desc) = description {
            form = form.text("track[description]", desc.to_string());
        }

        // Add artwork if provided
        if let Some(img_path) = image_path
            && img_path.exists()
        {
            let img_bytes = fs::read(img_path).context("Failed to read image file")?;

            let img_name = img_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("artwork.jpg")
                .to_string();

            let img_part = multipart::Part::bytes(img_bytes).file_name(img_name);

            form = form.part("track[artwork_data]", img_part);
        }

        // Add tags if provided
        if let Some(tag_list) = tags {
            let tags_string = tag_list.join(" ");
            form = form.text("track[tag_list]", tags_string);
        }

        // Set sharing to public
        form = form.text("track[sharing]", "public");

        debug!("Sending upload request...");

        // Send upload request with OAuth token
        let response = self
            .client
            .post(UPLOAD_URL)
            .header(
                "Authorization",
                format!("OAuth {}", token_info.access_token),
            )
            .multipart(form)
            .send()
            .context("Failed to upload file")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            bail!("Upload failed with status {}: {}", status, body);
        }

        // Get response text first for debugging
        let response_text = response.text().context("Failed to read response body")?;

        println!("\nSoundCloud API Response:");
        println!("{}", response_text);
        println!();

        let upload_response: UploadResponse =
            serde_json::from_str(&response_text).context("Failed to parse upload response")?;

        info!("Upload successful!");

        Ok(upload_response)
    }
}
