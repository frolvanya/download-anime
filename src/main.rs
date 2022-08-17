use core::time;
use std::io::Write;
use std::sync::mpsc::{channel, Receiver};
use std::thread::{self, JoinHandle};

use reqwest::blocking::Client;
use reqwest::header;

use select::document::Document;
use select::predicate::Name;

use clap::Parser;

/// Anime& Downloader
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
        println!("{}", response.status());
        println!("Bad response status ({} episode)! Trying again...", episode);
        std::thread::sleep(time::Duration::from_secs(2));
        download_episode(client, url, anime.clone(), episode);
    }

    let mut file = std::fs::File::create(format!("{}/episode-{}.mp4", anime, episode))
        .expect(&format!("Wasn't able to save `episode-{}.mp4`", episode));
    std::io::copy(&mut response, &mut file)
        .expect(&format!("Wasn't able to save `episode-{}.mp4`", episode));
}

fn loading_animation(episodes: u16, recv: Receiver<()>) {
    let frames = ["|", "/", "-", "\\"];

    if episodes == 1 {
        print!("Downloading 1 episode:  ");
    } else {
        print!("Downloading {} episodes:  ", episodes);
    }

    loop {
        for frame in frames {
            match recv.try_recv() {
                Ok(()) => {
                    println!("{}✔", 8u8 as char);
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

    let concurrency = args.concurrency;

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
            let (send, recv) = channel();

            thread::spawn(move || loading_animation(concurrency, recv));
            wait_for_threads(threads);

            send.send(())
                .expect("Couldn't send signal to the loading animation");
            threads = Vec::new();
        }
    }

    let (send, recv) = channel();

    let length = threads.len();
    thread::spawn(move || loading_animation(length as u16, recv));
    wait_for_threads(threads);

    send.send(())
        .expect("Couldn't send signal to the loading animation");
    std::thread::sleep(time::Duration::from_millis(200));
}
