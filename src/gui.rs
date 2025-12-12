use anyhow::Result;
use slint::SharedString;
use std::path::PathBuf;
use std::thread;

slint::include_modules!();

pub fn run_gui() -> Result<()> {
    let ui = MainWindow::new()?;

    // Handle file selection
    let ui_weak = ui.as_weak();
    ui.on_select_file(move || {
        let ui = ui_weak.unwrap();

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio", &["mp3", "m4a", "wav", "flac"])
            .pick_file()
        {
            ui.set_file_path(SharedString::from(path.display().to_string()));
        }
    });

    // Handle image selection
    let ui_weak = ui.as_weak();
    ui.on_select_image(move || {
        let ui = ui_weak.unwrap();

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Image", &["jpg", "jpeg", "png"])
            .pick_file()
        {
            ui.set_image_path(SharedString::from(path.display().to_string()));
        }
    });

    // Handle upload
    let ui_weak = ui.as_weak();
    ui.on_upload_clicked(move || {
        let ui = ui_weak.unwrap();

        // Get values from UI
        let file_path = ui.get_file_path().to_string();
        let title = ui.get_title_text().to_string();
        let description = ui.get_description_text().to_string();
        let image_path = ui.get_image_path().to_string();
        let tags = ui.get_tags_text().to_string();
        let mixcloud_enabled = ui.get_mixcloud_enabled();
        let soundcloud_enabled = ui.get_soundcloud_enabled();
        let schedule_enabled = ui.get_schedule_enabled();
        let schedule_date = ui.get_schedule_date().to_string();
        let schedule_time = ui.get_schedule_time().to_string();
        let generate_previews = ui.get_generate_previews();

        // Validate
        if file_path.is_empty() || title.is_empty() {
            ui.set_status_message(SharedString::from("Error: File and title are required"));
            return;
        }

        if !mixcloud_enabled && !soundcloud_enabled {
            ui.set_status_message(SharedString::from("Error: Select at least one platform"));
            return;
        }

        // Set uploading state
        ui.set_is_uploading(true);
        ui.set_status_message(SharedString::from("Uploading..."));

        // Spawn upload thread
        let ui_handle = ui.as_weak();
        thread::spawn(move || {
            let result = perform_upload(
                file_path,
                title,
                description,
                image_path,
                tags,
                mixcloud_enabled,
                soundcloud_enabled,
                schedule_enabled,
                schedule_date,
                schedule_time,
                generate_previews,
            );

            // Update UI with result
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_is_uploading(false);

                    match result {
                        Ok(message) => {
                            ui.set_status_message(SharedString::from(format!("✓ {}", message)));
                            ui.set_is_success(true);
                            ui.set_is_error(false);
                            // Clear form on success
                            ui.set_file_path(SharedString::from(""));
                            ui.set_title_text(SharedString::from(""));
                            ui.set_description_text(SharedString::from(""));
                            ui.set_image_path(SharedString::from(""));
                            ui.set_tags_text(SharedString::from(""));
                        }
                        Err(e) => {
                            ui.set_status_message(SharedString::from(format!("Error: {}", e)));
                            ui.set_is_success(false);
                            ui.set_is_error(true);
                        }
                    }
                }
            })
            .ok();
        });
    });

    ui.run()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn perform_upload(
    file_path: String,
    title: String,
    description: String,
    image_path: String,
    tags: String,
    mixcloud: bool,
    soundcloud: bool,
    schedule_enabled: bool,
    schedule_date: String,
    schedule_time: String,
    generate_previews: bool,
) -> Result<String> {
    use crate::platforms::{mixcloud, soundcloud as sc};
    use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

    let file = PathBuf::from(&file_path);

    // Generate preview snippets if requested
    if generate_previews {
        match crate::audio::create_preview_snippets(&file) {
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
    let image = if image_path.is_empty() {
        None
    } else {
        Some(PathBuf::from(&image_path))
    };

    let desc = if description.is_empty() {
        None
    } else {
        Some(description.as_str())
    };

    let tag_list = if tags.is_empty() {
        None
    } else {
        Some(
            tags.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    };

    // Parse and convert publish date to UTC if scheduled
    let publish_date = if schedule_enabled && !schedule_date.is_empty() && !schedule_time.is_empty()
    {
        // Parse date (YYYY-MM-DD)
        let date = NaiveDate::parse_from_str(&schedule_date, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("Invalid date format. Use YYYY-MM-DD: {}", e))?;

        // Parse time (HH:MM)
        let time = NaiveTime::parse_from_str(&schedule_time, "%H:%M")
            .map_err(|e| anyhow::anyhow!("Invalid time format. Use HH:MM: {}", e))?;

        // Combine date and time
        let naive_datetime = NaiveDateTime::new(date, time);

        // Convert local time to UTC
        let local_datetime = Local
            .from_local_datetime(&naive_datetime)
            .single()
            .ok_or_else(|| anyhow::anyhow!("Ambiguous local time"))?;

        let utc_datetime = local_datetime.with_timezone(&chrono::Utc);

        // Format as required: YYYY-MM-DDTHH:MM:SSZ
        Some(utc_datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string())
    } else {
        None
    };

    let mut results = Vec::new();

    // Upload to Mixcloud
    if mixcloud {
        let mut client = mixcloud::MixcloudClient::new()?;
        let response = client.upload(
            &file,
            &title,
            desc,
            image.as_deref(),
            tag_list.clone(),
            publish_date.as_deref(),
        )?;
        results.push(format!("Mixcloud: {}", response.result.message));
    }

    // Upload to SoundCloud
    if soundcloud {
        let mut client = sc::SoundcloudClient::new()?;
        let response = client.upload(&file, &title, desc, image.as_deref(), tag_list)?;
        results.push(format!("SoundCloud: Track #{}", response.id));
    }

    Ok(results.join(" | "))
}
