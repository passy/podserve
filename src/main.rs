use actix_web::{server, App, HttpRequest, Responder};
use id3;
use std::fs;
use std::path::Path;

#[derive(Debug)]
enum Error {
    IOError(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IOError(e)
    }
}

#[derive(Debug)]
struct PodData {
    artist: Option<String>,
}

fn greet(req: &HttpRequest) -> impl Responder {
    let to = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", to)
}

fn read_podcast_dir<P: AsRef<Path>>(path: P) -> Result<Vec<PodData>, std::io::Error> {
    Ok(fs::read_dir(path)?
        .filter_map(|p| p.ok())
        .map(|p| p.path())
        .map(id3::Tag::read_from_path)
        .filter_map(|id| id.ok())
        .map(|id| PodData {
            artist: id.artist().map(|s| s.to_owned()),
        })
        .collect::<Vec<_>>())
}

fn main() -> Result<(), std::io::Error> {
    let podcasts = read_podcast_dir("podcasts")?;
    server::new(|| {
        App::new()
            .resource("/", |r| r.f(greet))
            .resource("/{name}", |r| r.f(greet))
    })
    .bind("127.0.0.1:8000")
    .expect("Can not bind to port 8000")
    .run();

    Ok(())
}
