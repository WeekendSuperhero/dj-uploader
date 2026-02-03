pub mod mixcloud;
pub mod soundcloud;

use anyhow::Result;
use log::debug;
use std::path::Path;

use crate::cli::Platform;
use crate::config::TokenStorage;

/// Bring the app back to the foreground after an OAuth callback.
/// On macOS, this activates the app using AppleScript.
/// On other platforms, this is a no-op.
pub fn activate_app() {
    #[cfg(target_os = "macos")]
    {
        let pid = std::process::id();
        let script = format!(
            r#"tell application "System Events"
    set targetProcess to first application process whose unix id is {}
    set frontmost of targetProcess to true
end tell"#,
            pid
        );
        let result = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                debug!("Activated app via AppleScript");
            }
            _ => {
                // Fallback: try to activate by bundle ID for .app builds
                let _ = std::process::Command::new("open")
                    .args(["-b", "com.djuploader.app"])
                    .output();
            }
        }
    }
}

pub fn handle_auth(platform: Platform) -> Result<()> {
    match platform {
        Platform::Mixcloud => {
            mixcloud::MixcloudClient::authorize()?;
        }
        Platform::Soundcloud => {
            soundcloud::SoundcloudClient::authorize()?;
        }
    }
    Ok(())
}

pub fn handle_upload(
    platform: Platform,
    file_path: &Path,
    title: &str,
    description: Option<&str>,
    image_path: Option<&Path>,
    tags: Option<Vec<String>>,
    publish_date: Option<&str>,
) -> Result<()> {
    match platform {
        Platform::Mixcloud => {
            let mut client = mixcloud::MixcloudClient::new()?;
            let response = client.upload(
                file_path,
                title,
                description,
                image_path,
                tags,
                publish_date,
            )?;

            println!("\n✓ Upload successful!");
            println!("  Message: {}", response.result.message);
            println!("  Key: {}", response.result.key);
            println!("  URL: https://www.mixcloud.com{}", response.result.key);
            if publish_date.is_some() {
                println!("  Scheduled: Yes (check Mixcloud for publish time)");
            }
        }
        Platform::Soundcloud => {
            let mut client = soundcloud::SoundcloudClient::new()?;
            let response = client.upload(file_path, title, description, image_path, tags)?;

            println!("\n✓ Upload successful!");
            println!("  ID: {}", response.id);
            println!("  Title: {}", response.title);
            if let Some(url) = response.permalink_url {
                println!("  URL: {}", url);
            }
            if let Some(desc) = response.description {
                println!("  Description: {}", desc);
            }
        }
    }
    Ok(())
}

pub fn show_status() -> Result<()> {
    let token_storage = TokenStorage::load()?;

    println!("\n=== DJ Uploader Status ===\n");

    // Mixcloud status
    match &token_storage.mixcloud {
        Some(token_info) => {
            println!("Mixcloud: ✓ Authorized");
            println!(
                "  Token created: {}",
                token_info.created_at.format("%Y-%m-%d %H:%M:%S UTC")
            );

            if let Some(remaining) = token_info.time_until_expiry() {
                let days = remaining.num_days();
                let hours = remaining.num_hours() % 24;
                let minutes = remaining.num_minutes() % 60;

                if days > 0 {
                    println!("  Expires in: {} days, {} hours", days, hours);
                } else if hours > 0 {
                    println!("  Expires in: {} hours, {} minutes", hours, minutes);
                } else if minutes > 0 {
                    println!("  Expires in: {} minutes", minutes);
                } else {
                    println!("  Expires in: <1 minute (needs refresh)");
                }

                if token_info.is_expired() {
                    println!("  Status: ⚠️  Expired or expiring soon");
                    if token_info.refresh_token.is_some() {
                        println!("  Will auto-refresh on next upload");
                    } else {
                        println!("  Run 'dj-uploader auth mixcloud' to re-authorize");
                    }
                }
            } else {
                println!("  Expires: Unknown (no expiry info)");
            }
        }
        None => {
            println!("Mixcloud: ✗ Not authorized");
            println!("  Run 'dj-uploader auth mixcloud' to authorize");
        }
    }

    println!();

    // SoundCloud status
    match &token_storage.soundcloud {
        Some(token_info) => {
            println!("SoundCloud: ✓ Authorized");
            println!(
                "  Token created: {}",
                token_info.created_at.format("%Y-%m-%d %H:%M:%S UTC")
            );

            if let Some(remaining) = token_info.time_until_expiry() {
                let hours = remaining.num_hours();
                let minutes = remaining.num_minutes() % 60;

                if hours > 0 {
                    println!("  Expires in: {} hours, {} minutes", hours, minutes);
                } else if minutes > 0 {
                    println!("  Expires in: {} minutes", minutes);
                } else {
                    println!("  Expires in: <1 minute (needs refresh)");
                }

                if token_info.is_expired() {
                    println!("  Status: ⚠️  Expired or expiring soon");
                    if token_info.refresh_token.is_some() {
                        println!("  Will auto-refresh on next upload");
                    } else {
                        println!("  Run 'dj-uploader auth soundcloud' to re-authorize");
                    }
                }
            } else {
                println!("  Expires: Unknown (no expiry info)");
            }
        }
        None => {
            println!("SoundCloud: ✗ Not authorized");
            println!("  Run 'dj-uploader auth soundcloud' to authorize");
        }
    }

    println!("\nToken storage: {}", TokenStorage::token_path()?.display());

    Ok(())
}
