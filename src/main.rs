use core::time;
use std::io::Write;

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
}

async fn if_episode_exists(url: String) -> bool {
    let response = match reqwest::get(url).await {
        Ok(res) => res,
        Err(err) => {
            println!("Wasn't able to send request due to {}", err);
            return false;
        }
    };

    let text = match response.text().await {
        Ok(res) => res,
        Err(err) => {
            println!("Wasn't able to get text from response due to {}", err);
            return false;
        }
    };

    if text.contains(&"Страницы не существует или она была удалена.")
    {
        return false;
    }

    true
}

async fn get_video_urls(client: Client, url: String) -> Vec<String> {
    let response = match client.get(url).send() {
        Ok(res) => res,
        Err(err) => {
            println!("Wasn't able to send request due to {}", err);
            return Vec::new();
        }
    };

    let text = match response.text() {
        Ok(res) => res,
        Err(err) => {
            println!("Wasn't able to get text from response due to {}", err);
            return Vec::new();
        }
    };

    Document::from(text.as_str())
        .find(Name("source"))
        .filter_map(|link| link.attr("src"))
        .map(|link| link.to_string())
        .collect::<Vec<String>>()
}

fn download_episode(client: Client, url: String, anime: String, episode: u16) {
    let animation = tokio::spawn(loading_animation(episode));
    let mut response = match client.get(url).send() {
        Ok(res) => res,
        Err(err) => {
            println!("Wasn't able to send request due to {}", err);
            return;
        }
    };

    animation.abort();
    match std::fs::File::create(format!("{}/episode-{}.mp4", anime, episode)) {
        Ok(mut file) => match std::io::copy(&mut response, &mut file) {
            Ok(_) => {
                println!("{}✔", 8u8 as char);
            }
            Err(err) => {
                println!("{}✘", 8u8 as char);
                println!(
                    "Wasn't able to save `episode-{}.mp4` due to {}",
                    episode, err
                );
            }
        },
        Err(err) => {
            println!("{}✘", 8u8 as char);
            println!(
                "Wasn't able to save `episode-{}.mp4` due to {}",
                episode, err
            );
        }
    };
}

async fn loading_animation(episode: u16) {
    let frames = ["|", "/", "-", "\\"];
    print!("Episode {}:  ", episode);

    loop {
        for frame in frames {
            print!("{}{}", 8u8 as char, frame);
            std::io::stdout().flush().unwrap();
            std::thread::sleep(time::Duration::from_millis(100));
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let anime = args.anime;
    if !if_episode_exists(format!("https://jut.su/{}/episode-1.html", anime.clone())).await {
        println!("This anime doesn't exists");
        std::process::exit(1);
    };

    let episodes = match args.episodes.as_str() {
        "all episodes" => 1..9999,
        episodes_str => {
            let mut range = episodes_str
                .split('-')
                .map(|x| x.parse::<u16>().unwrap())
                .collect::<Vec<u16>>();
            if range[0] > range[1] {
                let tmp_right_range = range[0];
                range[0] = range[1];
                range[1] = tmp_right_range;
            }

            range[0]..range[1]
        }
    };

    let resolution = match args.resolution {
        360 => 3,
        480 => 2,
        720 => 1,
        1080 => 0,
        _ => {
            println!("Wrong resolution. Please, pick one of these: 360, 480, 720, 1080");
            std::process::exit(1);
        }
    };

    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.6 Safari/605.1.15"));
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();

    match std::fs::create_dir(anime.clone()) {
        Err(_) => println!("`{}` folder is already exists", anime.clone()),
        Ok(_) => println!("Created `{}` folder", anime.clone()),
    }

    // let mut download_episode_tasks = Vec::new();
    for episode in episodes {
        // let client_clone = client.clone();
        // let anime_clone = anime.clone();

        let url = format!("https://jut.su/{}/episode-{}.html", anime.clone(), episode);
        if !if_episode_exists(url.clone()).await {
            break;
        }

        let video_urls = get_video_urls(client.clone(), url.clone()).await;
        download_episode(
            client.clone(),
            video_urls[resolution].clone(),
            anime.clone(),
            episode,
        );

        // let animation = tokio::spawn(loading_animation(episode));
        // download_episode_tasks.push(tokio::spawn(async move {
        //     download_episode(
        //         client_clone,
        //         video_urls[resolution].clone(),
        //         anime_clone,
        //         episode,
        //     )
        //     .await;
        //     animation.abort();
        // }));

        // if download_episode_tasks.len() == 5 {
        //     for task in download_episode_tasks {
        //         task.await.expect("Panic in task")
        //     }

        //     download_episode_tasks = Vec::new();
        // }
    }
}
