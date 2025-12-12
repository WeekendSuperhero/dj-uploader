mod audio;
mod cli;
mod config;
mod gui;
mod platforms;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = cli::Cli::parse();

    // Check for GUI mode first
    if args.gui {
        return gui::run_gui();
    }

    // Initialize logging for CLI mode
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    match args.command {
        Some(cli::Commands::Auth { platform }) => {
            platforms::handle_auth(platform)?;
        }
        Some(cli::Commands::Upload {
            platform,
            file,
            title,
            description,
            image,
            tags,
            publish_date,
            generate_previews,
        }) => {
            let tag_list = tags.map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            });

            // Parse and convert publish_date if provided
            let publish_date_utc = if let Some(date_str) = publish_date {
                use chrono::{Local, NaiveDateTime, TimeZone};

                let naive_datetime = NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M")
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "Invalid publish_date format. Use 'YYYY-MM-DD HH:MM': {}",
                            e
                        )
                    })?;

                let local_datetime = Local
                    .from_local_datetime(&naive_datetime)
                    .single()
                    .ok_or_else(|| anyhow::anyhow!("Ambiguous local time"))?;

                let utc_datetime = local_datetime.with_timezone(&chrono::Utc);
                Some(utc_datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            } else {
                None
            };

            // Generate preview snippets if requested
            if generate_previews {
                match audio::create_preview_snippets(&file) {
                    Ok(snippets) => {
                        println!("✓ Generated {} preview snippets:", snippets.len());
                        for snippet in &snippets {
                            println!("  - {}", snippet.display());
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠ Warning: Failed to generate previews: {}", e);
                        // Continue with upload even if preview generation fails
                    }
                }
            }

            platforms::handle_upload(
                platform,
                &file,
                &title,
                description.as_deref(),
                image.as_deref(),
                tag_list,
                publish_date_utc.as_deref(),
            )?;
        }
        Some(cli::Commands::Status) => {
            platforms::show_status()?;
        }
        None => {
            println!("DJ Uploader - Upload your music to Mixcloud and SoundCloud");
            println!("\nUsage:");
            println!("  dj-uploader auth <platform>          Authorize with a platform");
            println!("  dj-uploader upload <platform> ...    Upload a mix");
            println!("  dj-uploader status                   Show configuration status");
            println!("\nUse --help for more information");
        }
    }

    Ok(())
}
