use core::time;
use std::io::Write;

use reqwest::blocking::Client;
use reqwest::header;

use select::document::Document;
use select::predicate::Name;

use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

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

fn download_episode(client: Client, send: Sender<bool>, url: String, anime: String, episode: u16) {
    let mut response = client.get(url).send().expect("Wasn't able to send request");

    match std::fs::File::create(format!("{}/episode-{}.mp4", anime, episode)) {
        Ok(mut file) => match std::io::copy(&mut response, &mut file) {
            Ok(_) => {
                send.send(true)
                    .expect("An error occurred while sending the signal");
                std::thread::sleep(time::Duration::from_secs(1));
            }
            Err(err) => {
                send.send(false)
                    .expect("An error occurred while sending the signal");
                std::thread::sleep(time::Duration::from_secs(1));

                println!(
                    "Wasn't able to save `episode-{}.mp4` due to {}",
                    episode, err
                );
            }
        },
        Err(err) => {
            send.send(false)
                .expect("An error occurred while sending the signal");
            std::thread::sleep(time::Duration::from_secs(1));

            println!(
                "Wasn't able to save `episode-{}.mp4` due to {}",
                episode, err
            );
        }
    };
}

fn loading_animation(episode: u16, recv: Receiver<bool>) {
    let frames = ["|", "/", "-", "\\"];
    print!("Episode {}:  ", episode);

    loop {
        for frame in frames {
            match recv.try_recv() {
                Ok(true) => {
                    println!("{}✔", 8u8 as char);
                    return;
                }
                Ok(false) => {
                    println!("{}✘", 8u8 as char);
                    return;
                }
                Err(_) => {}
            };

            print!("{}{}", 8u8 as char, frame);
            std::io::stdout().flush().unwrap();
            std::thread::sleep(time::Duration::from_millis(100));
        }
    }
}

#[tokio::main]
async fn main() {
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
        format!("https://jut.su/{}/episode-1.html", anime.clone()),
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
                let tmp_right_range = range[0];
                range[0] = range[1];
                range[1] = tmp_right_range;
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

    match std::fs::create_dir(anime.clone()) {
        Err(_) => println!("`{}` folder is already exists", anime.clone()),
        Ok(_) => println!("Created `{}` folder", anime.clone()),
    }

    for episode in episodes {
        let (send, recv) = channel();

        let url = format!("https://jut.su/{}/episode-{}.html", anime.clone(), episode);
        if !if_episode_exists(client.clone(), url.clone()) {
            break;
        }

        let video_urls = get_video_urls(client.clone(), url.clone());
        thread::spawn(move || loading_animation(episode, recv));
        download_episode(
            client.clone(),
            send,
            video_urls[resolution].clone(),
            anime.clone(),
            episode,
        );
    }
}
