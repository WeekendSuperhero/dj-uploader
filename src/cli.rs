use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dj-uploader")]
#[command(about = "Upload mixes to Mixcloud and SoundCloud", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Launch graphical user interface
    #[arg(long, global = true)]
    pub gui: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authorize with a platform
    Auth {
        /// Platform to authorize with
        #[arg(value_enum)]
        platform: Platform,
    },
    /// Upload a mix to a platform
    Upload {
        /// Platform to upload to
        #[arg(value_enum)]
        platform: Platform,

        /// Path to the audio file
        #[arg(short, long)]
        file: PathBuf,

        /// Title of the mix
        #[arg(short, long)]
        title: String,

        /// Description of the mix
        #[arg(short, long)]
        description: Option<String>,

        /// Path to cover image
        #[arg(short = 'i', long)]
        image: Option<PathBuf>,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Scheduled publish date in local time (format: YYYY-MM-DD HH:MM)
        /// Will be converted to UTC. Mixcloud Pro accounts only.
        #[arg(long)]
        publish_date: Option<String>,

        /// Generate preview snippets (30s, 60s, 90s) in the same folder
        #[arg(long)]
        generate_previews: bool,
    },
    /// Show current configuration status
    Status,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Platform {
    Mixcloud,
    Soundcloud,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Mixcloud => write!(f, "Mixcloud"),
            Platform::Soundcloud => write!(f, "SoundCloud"),
        }
    }
}
