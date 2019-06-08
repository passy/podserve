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

use id3;
use rocket::{get, response, routes, State};
use rocket_contrib::serve::StaticFiles;
use rss;
use std::fs;
use std::path::Path;
use chrono::{DateTime, offset::Utc};
use url;
use structopt::{StructOpt};
use std::time::SystemTime;
use pretty_env_logger;
use log;

#[derive(StructOpt, Debug)]
#[structopt(name = "podserve")]
struct Opt {
    base_url: url::Url,
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
    filename: String,
    timestamp: SystemTime,
    len: u64,
}

fn mkfeed(opt: &Opt, pods: &[PodData]) -> Result<rss::Channel, String> {
    rss::ChannelBuilder::default()
        .title("PodServe Feed")
        .description(format!("An RSS feed generated by PodServe {}.", env!("CARGO_PKG_VERSION")))
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
        .description("".to_string())
        .guid(
            rss::GuidBuilder::default()
                .value(filename)
                .build()?,
        )
        .enclosure(
            rss::EnclosureBuilder::default()
                .url(full_url)
                // TODO: Ensure that this is true while reading directory.
                .mime_type("audio/mpeg")
                // TODO: Add time/length here.
                .build()?
        )
        .pub_date(format_systemtime(&pd.timestamp))
        .build()
        .map_err(|e| e.into())
}

#[get("/")]
fn index(podcasts: State<PodcastState>, opt: State<Opt>) -> Result<response::content::Xml<String>, String> {
    Ok(response::content::Xml(mkfeed(&opt, &podcasts.0)?.to_string()))
}

fn read_podcast_dir<P: AsRef<Path>>(path: P) -> Result<Vec<PodData>, std::io::Error> {
    Ok(fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|p| p.path())
        .filter_map(|p| id3::Tag::read_from_path(&p).map(|t| (p, t)).ok())
        .map(|(path, tag): (std::path::PathBuf, id3::Tag)| PodData {
            artist: tag.artist().map(ToOwned::to_owned),
            title: tag.title().map(ToOwned::to_owned),
            filename: path
                .file_name()
                .and_then(|s| s.to_str())
                .expect("Valid filename")
                .to_string(),
            timestamp: path.metadata().and_then(|m| m.created()).unwrap_or_else(|e| {
                log::warn!("Failed to obtain created timestamp for {:?}: {}", &path, e);
                SystemTime::now()
            }),
            len: path.metadata().map(|m| m.len()).unwrap_or_else(|e| {
                log::warn!("Unable to determine file length for {:?}: {}", &path, e);
                0
            })
        })
        .collect::<Vec<_>>())
}

fn rocket(opt: Opt) -> Result<rocket::Rocket, std::io::Error> {
    let podcasts = PodcastState(read_podcast_dir("podcasts")?);

    Ok(rocket::ignite()
        .manage(podcasts)
        .manage(opt)
        .mount("/", routes![index])
        // TODO: Make this configurable.
        .mount(
            "/podcasts",
            StaticFiles::from(concat!(env!("CARGO_MANIFEST_DIR"), "/podcasts")),
        ))
}

fn main() -> Result<(), std::io::Error> {
    pretty_env_logger::try_init().unwrap();
    let opt = Opt::from_args();
    rocket(opt)?.launch();
    Ok(())
}
