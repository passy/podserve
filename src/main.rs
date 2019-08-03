#![warn(
    clippy::all,
    clippy::restriction,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]
#![allow(
    clippy::missing_docs_in_private_items,
    clippy::implicit_return,
    clippy::filter_map
)]
#![feature(proc_macro_hygiene, decl_macro)]

use chrono::{offset::Utc, DateTime};
use id3;
use log;
use pretty_env_logger;
use rocket::{get, response, routes, State};
use rocket_contrib::serve::StaticFiles;
use rss;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use structopt::StructOpt;
use url;

mod config;

#[derive(StructOpt, Debug)]
#[structopt(name = "podserve")]
struct Opt {
    base_url: url::Url,
    #[structopt(short = "d", long = "directory", default_value = "podcasts")]
    /// Directory to serve podcast MP3 files from.
    directory: PathBuf,
    #[structopt(long = "write-config")]
    /// Write a default configuration file to the given path an exit.
    write_config: Option<PathBuf>,
    #[structopt(long = "config")]
    /// Read a config file from `config`. To create a default config use `--write-config`.
    config: Option<PathBuf>,
}

#[derive(Debug)]
enum RunMode<'a> {
    Serve,
    WriteConfig(&'a PathBuf),
}

#[derive(Debug)]
enum Error {
    IOError(std::io::Error),
    URLParseError(url::ParseError),
    GenericError(String),
}

struct PodcastState(Vec<PodData>);

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IOError(e)
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::GenericError(e)
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Error::URLParseError(e)
    }
}

#[derive(Debug)]
struct PodData {
    artist: Option<String>,
    title: Option<String>,
    comment: Option<String>,
    filename: String,
    timestamp: SystemTime,
    len: u64,
}

fn mkfeed(opt: &Opt, config: &config::Config, pods: &[PodData]) -> Result<rss::Channel, String> {
    rss::ChannelBuilder::default()
        .title(&config.title)
        .description(&config.description)
        .items(
            pods.iter()
                .map(|i| mkitem(opt, i))
                .filter_map(Result::ok)
                .collect::<Vec<_>>(),
        )
        .build()
}

fn format_systemtime(t: &SystemTime) -> String {
    let datetime: DateTime<Utc> = t.clone().into();
    datetime.to_rfc2822()
}

fn mkitem(opt: &Opt, pd: &PodData) -> Result<rss::Item, Error> {
    let filename = pd.filename.clone();
    let full_url_res = opt.base_url.clone().join("/podcasts/")?.join(&filename)?;
    let full_url = full_url_res.as_str();
    rss::ItemBuilder::default()
        .title(pd.title.clone())
        .description(pd.comment.clone().unwrap_or("".to_string()))
        .guid(rss::GuidBuilder::default().value(filename).build()?)
        .enclosure(
            rss::EnclosureBuilder::default()
                .url(full_url)
                // TODO: Ensure that this is true while reading directory.
                .mime_type("audio/mpeg")
                .length(format!("{}", pd.len))
                .build()?,
        )
        .pub_date(format_systemtime(&pd.timestamp))
        .build()
        .map_err(|e| e.into())
}

#[get("/")]
fn index(
    podcasts: State<PodcastState>,
    config: State<config::Config>,
    opt: State<Opt>,
) -> Result<response::content::Xml<String>, String> {
    Ok(response::content::Xml(
        mkfeed(&opt, &config, &podcasts.0)?.to_string(),
    ))
}

fn read_podcast_dir<P: AsRef<Path>>(path: P) -> Result<Vec<PodData>, std::io::Error> {
    let filename = |path: &Path| {
        path.file_name()
            .and_then(|s| s.to_str())
            .expect("Valid filename")
            .to_string()
    };
    let timestamp = |path: &Path| {
        path.metadata()
            .and_then(|m| m.modified())
            .unwrap_or_else(|e| {
                log::warn!("Failed to obtain created timestamp for {:?}: {}", &path, e);
                SystemTime::now()
            })
    };
    let len = |path: &Path| {
        path.metadata().map(|m| m.len()).unwrap_or_else(|e| {
            log::warn!("Unable to determine file length for {:?}: {}", &path, e);
            0
        })
    };
    Ok(fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|p| p.path())
        .map(|p| {
            id3::Tag::read_from_path(&p)
                .map(|t| (p.clone(), t))
                .map_err(|e| (p, e))
        })
        .map(|t| match t {
            Ok((path, tag)) => PodData {
                artist: tag.artist().map(ToOwned::to_owned),
                title: tag.title().map(ToOwned::to_owned),
                comment: Some(
                    tag.comments()
                        .map(|c| c.text.to_string())
                        .collect::<Vec<_>>()
                        .concat(),
                ),
                filename: filename(&path),
                timestamp: timestamp(&path),
                len: len(&path),
            },
            Err((path, _)) => PodData {
                artist: None,
                title: Some(filename(&path)),
                comment: None,
                filename: filename(&path),
                timestamp: timestamp(&path),
                len: len(&path),
            },
        })
        .collect::<Vec<_>>())
}

fn rocket(config: config::Config, opt: Opt) -> Result<rocket::Rocket, std::io::Error> {
    let podcasts = PodcastState(read_podcast_dir(&opt.directory)?);
    let cwd = env::current_dir()?;

    Ok(rocket::ignite()
        .manage(podcasts)
        .manage(config)
        .mount("/", routes![index])
        .mount("/podcasts", StaticFiles::from(cwd.join(&opt.directory)))
        .manage(opt))
}

fn mode_from_opt<'a>(opt: &'a Opt) -> RunMode<'a> {
    if let Some(path) = &opt.write_config {
        RunMode::WriteConfig(path)
    } else {
        RunMode::Serve
    }
}

fn main() -> Result<(), failure::Error> {
    pretty_env_logger::try_init().unwrap();
    let opt = Opt::from_args();
    match mode_from_opt(&opt) {
        RunMode::Serve => {
            let config = opt.config.as_ref().map(|f| config::read_config(&f).expect("Valid config")).unwrap_or_else(|| Default::default());
            let _ = rocket(config, opt)?.launch();
        }
        RunMode::WriteConfig(path) => {
            config::write_config(&Default::default(), path)?;
            eprintln!("Config written to '{}'", path.to_str().expect("Valid path"));
        }
    }
    Ok(())
}
