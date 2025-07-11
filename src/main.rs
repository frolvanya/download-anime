use std::{str::FromStr, sync::Arc};

use clap::Parser;

use downloader::Downloader;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod downloader;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Invalid episodes format: {0}")]
    InvalidEpisodesFormat(String),

    #[error("Invalid resolution format: {0}")]
    InvalidResolutionFormat(String),

    #[error("Request error: {0:?}")]
    RequestError(#[from] reqwest::Error),

    #[error("IO error: {0:?}")]
    IoError(#[from] std::io::Error),

    #[error("No video links found for episode: {0}")]
    NoVideoLinksFound(usize),

    #[error("Logging error: {0:?}")]
    InitLoggerError(#[from] tracing::dispatcher::SetGlobalDefaultError),

    #[error("Failed to join task: {0:?}")]
    JoinError(#[from] tokio::task::JoinError),
}

#[derive(Debug, Copy, Clone)]
enum Episodes {
    AllEpisodes,
    Range(usize, usize),
}

impl Episodes {
    fn iter(&self) -> EpisodesIter {
        match self {
            Episodes::AllEpisodes => EpisodesIter::All(1),
            Episodes::Range(start, end) => EpisodesIter::Range(*start, *end),
        }
    }
}

#[derive(Debug)]
enum EpisodesIter {
    All(usize),
    Range(usize, usize),
}

impl Iterator for EpisodesIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EpisodesIter::All(current) => {
                let result = *current;
                *current += 1;
                Some(result)
            }
            EpisodesIter::Range(current, end) => {
                if *current <= *end {
                    let result = *current;
                    *current += 1;
                    Some(result)
                } else {
                    None
                }
            }
        }
    }
}

impl FromStr for Episodes {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "all" {
            Ok(Episodes::AllEpisodes)
        } else if let Some((start, end)) = s.split_once('-') {
            let start = start.parse().map_err(|err| {
                Self::Err::InvalidEpisodesFormat(format!("Invalid start: {err:?}"))
            })?;
            let end = end
                .parse()
                .map_err(|err| Self::Err::InvalidEpisodesFormat(format!("Invalid end: {err:?}")))?;

            Ok(Episodes::Range(start, end))
        } else {
            Err(Self::Err::InvalidEpisodesFormat(
                "Expected format: 'all' or 'start-end'".to_owned(),
            ))
        }
    }
}

impl std::fmt::Display for Episodes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Episodes::AllEpisodes => write!(f, "all"),
            Episodes::Range(start, end) => write!(f, "{start}-{end}"),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum Resolution {
    FullHD,
    HD,
    SD,
}

impl FromStr for Resolution {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fullhd" | "1080p" => Ok(Resolution::FullHD),
            "hd" | "720p" => Ok(Resolution::HD),
            "sd" | "480p" => Ok(Resolution::SD),
            _ => Err(Self::Err::InvalidResolutionFormat(
                "Expected one of: 'fullhd', 'hd', 'sd', '1080p', '720p', '480p'".to_owned(),
            )),
        }
    }
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resolution::FullHD => write!(f, "fullhd"),
            Resolution::HD => write!(f, "hd"),
            Resolution::SD => write!(f, "sd"),
        }
    }
}

impl From<Resolution> for usize {
    fn from(val: Resolution) -> Self {
        match val {
            Resolution::FullHD => 0,
            Resolution::HD => 1,
            Resolution::SD => 2,
        }
    }
}

/// Anime Downloader
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Anime name
    #[clap(short, long)]
    anime: String,

    /// Anime episodes to download
    #[clap(short, long, default_value_t = Episodes::AllEpisodes)]
    episodes: Episodes,

    /// Specifies episodes resolution
    #[clap(short, long, default_value_t = Resolution::FullHD)]
    resolution: Resolution,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();

    let downloader = Arc::new(Downloader::new(args.anime, args.episodes, args.resolution)?);
    downloader.run().await
}
