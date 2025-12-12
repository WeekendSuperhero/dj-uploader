use anyhow::{Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// Creates preview snippets of an audio file at 30, 60, and 90 seconds
/// Each snippet takes 10-second chunks from intro, middle, and end with fade effects
pub fn create_preview_snippets(file_path: &Path) -> Result<Vec<PathBuf>> {
    let durations = vec![30, 60, 90]; // seconds
    let mut output_files = Vec::new();

    // Get the total duration first
    let total_duration = get_audio_duration(file_path)?;

    for duration in durations {
        let output_path = generate_snippet_path(file_path, duration)?;
        create_snippet(file_path, &output_path, duration, total_duration)?;
        output_files.push(output_path);
    }

    Ok(output_files)
}

/// Generate output path for snippet
fn generate_snippet_path(original: &Path, duration: u64) -> Result<PathBuf> {
    let parent = original.parent().unwrap_or(Path::new("."));
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid file name")?;

    // Always output as WAV to avoid encoding complexity
    let output_name = format!("{}_preview_{}s.wav", stem, duration);
    Ok(parent.join(output_name))
}

/// Get the duration of an audio file in seconds
fn get_audio_duration(file_path: &Path) -> Result<f64> {
    let file = File::open(file_path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .context("Failed to probe audio file")?;

    let format = probed.format;
    let track = format
        .default_track()
        .context("No default audio track found")?;

    let time_base = track.codec_params.time_base;
    let n_frames = track.codec_params.n_frames;

    if let (Some(tb), Some(frames)) = (time_base, n_frames) {
        Ok(tb.calc_time(frames).seconds as f64)
    } else {
        // Fallback
        Ok(180.0) // Assume 3 minutes if we can't determine
    }
}

/// Create a snippet from the audio file
/// Takes 10-second chunks from intro, middle, and end with fade effects
fn create_snippet(
    input_path: &Path,
    output_path: &Path,
    duration_secs: u64,
    total_duration: f64,
) -> Result<()> {
    let chunk_duration = 10.0; // Always 10 seconds per chunk
    let num_chunks = (duration_secs as f64 / chunk_duration) as usize;

    // Calculate start positions for each chunk
    let mut positions = Vec::new();

    match num_chunks {
        3 => {
            // 30s: intro (0s), middle, end
            positions.push(0.0);
            positions.push((total_duration / 2.0) - (chunk_duration / 2.0));
            positions.push((total_duration - chunk_duration).max(20.0));
        }
        6 => {
            // 60s: 2 chunks from intro, 2 from middle, 2 from end
            positions.push(0.0);
            positions.push(10.0);
            positions.push((total_duration / 2.0) - chunk_duration);
            positions.push(total_duration / 2.0);
            positions.push((total_duration - (2.0 * chunk_duration)).max(40.0));
            positions.push((total_duration - chunk_duration).max(50.0));
        }
        9 => {
            // 90s: 3 chunks from intro, 3 from middle, 3 from end
            positions.push(0.0);
            positions.push(10.0);
            positions.push(20.0);
            positions.push((total_duration / 2.0) - (1.5 * chunk_duration));
            positions.push((total_duration / 2.0) - (0.5 * chunk_duration));
            positions.push((total_duration / 2.0) + (0.5 * chunk_duration));
            positions.push((total_duration - (3.0 * chunk_duration)).max(60.0));
            positions.push((total_duration - (2.0 * chunk_duration)).max(70.0));
            positions.push((total_duration - chunk_duration).max(80.0));
        }
        _ => {
            anyhow::bail!("Unsupported duration: {}s", duration_secs);
        }
    }

    // Extract all chunks
    let mut all_samples = Vec::new();
    let mut sample_rate = 44100;

    for &start_pos in &positions {
        let (samples, sr) = extract_chunk(input_path, start_pos, chunk_duration)?;
        sample_rate = sr;

        // Apply fade in/out
        let faded = apply_fades(samples, sample_rate);
        all_samples.extend(faded);
    }

    // Write to WAV file
    write_wav(output_path, &all_samples, sample_rate)?;

    Ok(())
}

/// Extract a chunk of audio starting at a specific position
fn extract_chunk(
    input_path: &Path,
    start_secs: f64,
    duration_secs: f64,
) -> Result<(Vec<f32>, u32)> {
    let file = File::open(input_path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = input_path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .context("Failed to probe audio file")?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .context("No default audio track found")?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    // Seek to start position
    let seek_to = SeekTo::Time {
        time: Time::from(start_secs),
        track_id: Some(track_id),
    };

    let _ = format.seek(SeekMode::Accurate, seek_to);

    let mut samples = Vec::new();
    let target_samples = (duration_secs * sample_rate as f64) as usize;

    while samples.len() < target_samples {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let chunk = convert_to_f32_mono(&decoded);
                samples.extend(chunk);
            }
            Err(_) => continue,
        }
    }

    // Trim to exact length
    samples.truncate(target_samples);

    Ok((samples, sample_rate))
}

/// Convert AudioBufferRef to mono f32 samples
fn convert_to_f32_mono(decoded: &AudioBufferRef) -> Vec<f32> {
    use symphonia::core::conv::FromSample;

    match decoded {
        AudioBufferRef::F32(buf) => {
            // Mix to mono if stereo
            if buf.spec().channels.count() == 2 {
                let left = buf.chan(0);
                let right = buf.chan(1);
                left.iter()
                    .zip(right.iter())
                    .map(|(&l, &r)| (l + r) / 2.0)
                    .collect()
            } else {
                buf.chan(0).to_vec()
            }
        }
        AudioBufferRef::S16(buf) => {
            if buf.spec().channels.count() == 2 {
                let left = buf.chan(0);
                let right = buf.chan(1);
                left.iter()
                    .zip(right.iter())
                    .map(|(&l, &r)| (f32::from_sample(l) + f32::from_sample(r)) / 2.0)
                    .collect()
            } else {
                buf.chan(0).iter().map(|&s| f32::from_sample(s)).collect()
            }
        }
        AudioBufferRef::S32(buf) => {
            if buf.spec().channels.count() == 2 {
                let left = buf.chan(0);
                let right = buf.chan(1);
                left.iter()
                    .zip(right.iter())
                    .map(|(&l, &r)| (f32::from_sample(l) + f32::from_sample(r)) / 2.0)
                    .collect()
            } else {
                buf.chan(0).iter().map(|&s| f32::from_sample(s)).collect()
            }
        }
        _ => vec![], // Fallback for other formats
    }
}

/// Apply 1-second fade in and fade out
fn apply_fades(mut samples: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    let fade_samples = sample_rate as usize; // 1 second
    let len = samples.len();

    if len <= fade_samples * 2 {
        return samples;
    }

    // Fade in
    for (i, sample) in samples.iter_mut().enumerate().take(fade_samples) {
        let factor = i as f32 / fade_samples as f32;
        *sample *= factor;
    }

    // Fade out
    for (i, sample) in samples.iter_mut().enumerate().take(fade_samples) {
        let factor = 1.0 - (i as f32 / fade_samples as f32);
        *sample *= factor;
    }

    samples
}

/// Write samples to WAV file
fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec).context("Failed to create WAV writer")?;

    for &sample in samples {
        let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }

    writer.finalize()?;

    Ok(())
}
