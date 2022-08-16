use std::thread::{self, JoinHandle};

use reqwest::blocking::Client;
use reqwest::header;

use select::document::Document;
use select::predicate::Name;

use clap::Parser;

/// Anime Downloader
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Anime name
    #[clap(short, long, value_parser)]
    anime: String,

    /// Anime episodes to download
    #[clap(short, long, value_parser, default_value_t = String::from("all episodes"))]
    episodes: String,

    /// Specifies episodes resolution
    #[clap(short, long, value_parser, default_value_t = 1080)]
    resolution: u16,

    /// Specifies concurrency
    #[clap(short, long, value_parser, default_value_t = 12)]
    concurrency: u16,
}

fn if_episode_exists(client: Client, url: String) -> bool {
    let response = client.get(url).send().expect("Wasn't able to send request");
    let text = response
        .text()
        .expect("Wasn't able to get text from response");

    if text.contains(&"Страницы не существует или она была удалена.")
    {
        return false;
    }

    true
}

fn get_video_urls(client: Client, url: String) -> Vec<String> {
    let response = client.get(url).send().expect("Wasn't able to send request");
    let text = response
        .text()
        .expect("Wasn't able to get text from response");

    Document::from(text.as_str())
        .find(Name("source"))
        .filter_map(|link| link.attr("src"))
        .map(|link| link.to_string())
        .collect::<Vec<String>>()
}

fn download_episode(client: Client, url: String, anime: String, episode: u16) {
    let mut response = client
        .get(url.clone())
        .send()
        .expect("Wasn't able to send request");

    if !response.status().is_success() {
        println!("Bad response status! Trying again...");
        download_episode(client, url, anime.clone(), episode);
    }

    match std::fs::File::create(format!("{}/episode-{}.mp4", anime, episode)) {
        Ok(mut file) => match std::io::copy(&mut response, &mut file) {
            Ok(_) => {
                println!("Episode {}: ✔", episode);
            }
            Err(err) => {
                println!("Episode {}: ✘", episode);

                println!(
                    "Wasn't able to save `episode-{}.mp4` due to {}",
                    episode, err
                );
            }
        },
        Err(err) => {
            println!("Episode {}: ✘", episode);

            println!(
                "Wasn't able to save `episode-{}.mp4` due to {}",
                episode, err
            );
        }
    };
}

fn wait_for_threads(threads: Vec<JoinHandle<()>>) {
    for child_thread in threads {
        child_thread.join().unwrap();
    }
}

fn main() {
    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.6 Safari/605.1.15"));
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();

    let args = Args::parse();

    let anime = args.anime;
    if !if_episode_exists(
        client.clone(),
        format!("https://jut.su/{}/episode-1.html", anime),
    ) {
        panic!("This anime doesn't exists");
    };

    let episodes = match args.episodes.as_str() {
        "all episodes" => 1..9999,
        episodes_str => {
            let mut range = episodes_str
                .split('-')
                .map(|x| x.parse::<u16>().unwrap())
                .collect::<Vec<u16>>();
            if range[0] > range[1] {
                range.swap(0, 1);
            }

            range[0]..range[1] + 1
        }
    };

    let resolution = match args.resolution {
        360 => 3,
        480 => 2,
        720 => 1,
        1080 => 0,
        _ => panic!("Wrong resolution. Please, pick one of these: 360, 480, 720, 1080"),
    };

    let concurrency = args.resolution;

    match std::fs::create_dir(anime.clone()) {
        Err(_) => println!("`{}` folder is already exists", anime),
        Ok(_) => println!("Created `{}` folder", anime),
    }

    let mut threads = Vec::new();
    for episode in episodes {
        let url = format!("https://jut.su/{}/episode-{}.html", anime.clone(), episode);
        if !if_episode_exists(client.clone(), url.clone()) {
            break;
        }

        let video_urls = get_video_urls(client.clone(), url.clone());
        let (client_clone, anime_clone) = (client.clone(), anime.clone());

        threads.push(thread::spawn(move || {
            download_episode(
                client_clone,
                video_urls[resolution].clone(),
                anime_clone,
                episode,
            );
        }));

        if episode % concurrency == 0 {
            wait_for_threads(threads);
            threads = Vec::new();
        }
    }

    wait_for_threads(threads);
}
