use anyhow::{Context, Result, bail};
use log::{debug, info, warn};
use reqwest::blocking::{Client, multipart};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use url::Url;

use crate::config::{MixcloudCredentials, TokenInfo, TokenStorage};

const OAUTH_AUTHORIZE_URL: &str = "https://www.mixcloud.com/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://www.mixcloud.com/oauth/access_token";
const UPLOAD_URL: &str = "https://api.mixcloud.com/upload/";
const REDIRECT_URI: &str = "http://localhost:8888/callback";

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
    pub result: UploadResult,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadResult {
    pub success: bool,
    pub message: String,
    pub key: String,
}

pub struct MixcloudClient {
    client: Client,
    credentials: MixcloudCredentials,
    token_storage: TokenStorage,
}

impl MixcloudClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .context("Failed to create HTTP client")?;

        let credentials = MixcloudCredentials::new();
        let token_storage = TokenStorage::load()?;

        Ok(Self {
            client,
            credentials,
            token_storage,
        })
    }

    pub fn authorize() -> Result<()> {
        info!("Starting Mixcloud OAuth2 authorization...");

        let credentials = MixcloudCredentials::new();

        // Build authorization URL
        let mut auth_url = Url::parse(OAUTH_AUTHORIZE_URL)?;
        auth_url
            .query_pairs_mut()
            .append_pair("client_id", &credentials.client_id)
            .append_pair("redirect_uri", REDIRECT_URI);

        println!("\nOpening browser for authorization...");
        println!("If the browser doesn't open, visit this URL:\n");
        println!("{}\n", auth_url);

        // Open browser
        if let Err(e) = webbrowser::open(auth_url.as_str()) {
            eprintln!("Failed to open browser: {}", e);
        }

        // Start local server to receive callback
        let listener = TcpListener::bind("127.0.0.1:8888")
            .context("Failed to start callback server. Is port 8888 already in use?")?;

        println!("Waiting for authorization...");

        let (mut stream, _) = listener.accept()?;
        let buf_reader = BufReader::new(&stream);
        let request_line = buf_reader
            .lines()
            .next()
            .context("Failed to read request")?
            .context("Empty request")?;

        // Parse the authorization code from the request
        let code = Self::extract_code_from_request(&request_line)?;

        // Send success response to browser with auto-close script
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
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
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
        // Auto-close after 2 seconds
        setTimeout(function() {
            window.close();
        }, 2000);
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
        let client = Client::new();
        let mut params = HashMap::new();
        params.insert("client_id", credentials.client_id.clone());
        params.insert("client_secret", credentials.client_secret.clone());
        params.insert("redirect_uri", REDIRECT_URI.to_string());
        params.insert("code", code);

        let response = client
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
        storage.set_mixcloud_tokens(token_info);
        storage.save()?;

        println!("\n✓ Authorization successful!");
        println!("Token saved to: {}", TokenStorage::token_path()?.display());

        if let Some(expires_in) = token_response.expires_in {
            let hours = expires_in / 3600;
            let days = hours / 24;
            if days > 0 {
                println!("Token expires in {} days", days);
            } else {
                println!("Token expires in {} hours", hours);
            }
        }

        println!("\nYou can now upload mixes with:");
        println!("  dj-uploader upload mixcloud --file <path> --title \"Your Mix\"");

        Ok(())
    }

    fn extract_code_from_request(request_line: &str) -> Result<String> {
        // Request line looks like: GET /callback?code=AUTH_CODE HTTP/1.1
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

        Ok(code)
    }

    fn refresh_token_if_needed(&mut self) -> Result<()> {
        let token_info = self.token_storage.get_mixcloud_token()?;

        if token_info.is_expired() {
            warn!("Access token is expired or expiring soon, refreshing...");

            let refresh_token = token_info.refresh_token.as_ref().context(
                "No refresh token available. Please re-authorize with 'dj-uploader auth mixcloud'",
            )?;

            let mut params = HashMap::new();
            params.insert("client_id", self.credentials.client_id.clone());
            params.insert("client_secret", self.credentials.client_secret.clone());
            params.insert("grant_type", "refresh_token".to_string());
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

            self.token_storage.set_mixcloud_tokens(new_token_info);
            self.token_storage.save()?;

            info!("Token refreshed successfully");
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upload(
        &mut self,
        file_path: &Path,
        title: &str,
        description: Option<&str>,
        image_path: Option<&Path>,
        tags: Option<Vec<String>>,
        publish_date: Option<&str>,
    ) -> Result<UploadResponse> {
        // Check if we have a token, if not, authorize first
        if self.token_storage.mixcloud.is_none() {
            println!("\nNo authorization found. Starting OAuth2 flow...\n");
            Self::authorize()?;
            // Reload token storage after authorization
            self.token_storage = TokenStorage::load()?;
        }

        // Refresh token if needed
        self.refresh_token_if_needed()?;

        let token_info = self.token_storage.get_mixcloud_token()?;

        info!("Uploading {} to Mixcloud...", file_path.display());

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

        form = form.part("mp3", file_part);

        // Add metadata
        form = form.text("name", title.to_string());

        if let Some(desc) = description {
            form = form.text("description", desc.to_string());
        }

        // Add cover image if provided
        if let Some(img_path) = image_path
            && img_path.exists() {
                let img_bytes = fs::read(img_path).context("Failed to read image file")?;

                let img_name = img_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("cover.jpg")
                    .to_string();

                let img_part = multipart::Part::bytes(img_bytes).file_name(img_name);

                form = form.part("picture", img_part);
            }

        // Add tags if provided (Mixcloud expects tags-0-tag, tags-1-tag, etc.)
        if let Some(tag_list) = tags {
            for (index, tag) in tag_list.iter().enumerate() {
                let field_name = format!("tags-{}-tag", index);
                form = form.text(field_name, tag.to_string());
            }
        }

        // Add publish_date if provided (Pro accounts only)
        if let Some(date) = publish_date {
            form = form.text("publish_date", date.to_string());
            debug!("Scheduling publish for: {}", date);
        }

        debug!("Sending upload request...");

        // Send upload request with OAuth token
        let response = self
            .client
            .post(UPLOAD_URL)
            .query(&[("access_token", &token_info.access_token)])
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

        // Always print the response so we can see what Mixcloud returns
        println!("\nMixcloud API Response:");
        println!("{}", response_text);
        println!();

        let upload_response: UploadResponse =
            serde_json::from_str(&response_text).context("Failed to parse upload response")?;

        info!("Upload successful!");

        Ok(upload_response)
    }
}
