use std::{path::Path, sync::Arc};

use futures::future::try_join_all;
use reqwest::{header, Client, Url};
use select::{document::Document, predicate::Name};
use tracing::info;

use crate::{Episodes, Error, Resolution};

pub struct Downloader {
    client: Client,
    anime: String,
    episodes: Episodes,
    resolution: Resolution,
}

impl Downloader {
    pub fn new(anime: String, episodes: Episodes, resolution: Resolution) -> Result<Self, Error> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.6 Safari/605.1.15"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Downloader {
            client,
            anime,
            episodes,
            resolution,
        })
    }

    fn construct_url(&self, episode: usize) -> String {
        format!("https://jut.su/{}/episode-{episode}.html", self.anime,)
    }

    async fn does_episode_exist(&self, episode: usize) -> Result<bool, Error> {
        let url = self.construct_url(episode);
        let response = self.client.get(url).send().await?;
        let text = response.text().await?;

        Ok(!text.contains("Страницы не существует или она была удалена."))
    }

    async fn get_video_urls(&self, episode: usize) -> Result<Vec<Url>, Error> {
        let url = self.construct_url(episode);
        let response = self.client.get(url).send().await?;
        let text = response.text().await?;

        Ok(Document::from(text.as_str())
            .find(Name("source"))
            .filter_map(|link| link.attr("src").and_then(|link| link.parse().ok()))
            .collect())
    }

    async fn download_episode(&self, url: Url, episode: usize) -> Result<(), Error> {
        info!("Downloading episode #{episode}");

        let response = self.client.get(url).send().await?;
        let bytes = response.bytes().await?;

        let path = format!("{}/episode-{episode}.mp4", self.anime);
        tokio::fs::write(path, &bytes).await?;

        Ok(())
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Error> {
        info!("Starting download for anime: {}", self.anime);

        let dir = Path::new(&self.anime);
        if !dir.exists() {
            tokio::fs::create_dir_all(dir).await?;
        }

        let mut handles = Vec::new();

        for episode in self.episodes.iter() {
            if !self.does_episode_exist(episode).await? {
                info!("Episode {episode} does not exist, stopping here");
                break;
            }

            handles.push(tokio::spawn({
                let cloned_self = self.clone();
                async move {
                    let video_urls = cloned_self.get_video_urls(episode).await?;

                    if video_urls.is_empty() {
                        return Err(Error::NoVideoLinksFound(episode));
                    }

                    let resolution_url = video_urls
                        .get(usize::from(cloned_self.resolution))
                        .cloned()
                        .ok_or_else(|| Error::NoVideoLinksFound(episode))?;

                    cloned_self.download_episode(resolution_url, episode).await
                }
            }));
        }

        try_join_all(handles).await?;

        info!(
            "Successfully downloaded {} episodes of {}",
            self.episodes, self.anime
        );

        Ok(())
    }
}
