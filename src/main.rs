use std::error::Error;
use std::io::Cursor;
use std::path::PathBuf;
use clap::Parser;
use env_logger::{Builder, WriteStyle};
use futures::StreamExt;
use log::{error,info, LevelFilter};
use reqwest::Url;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;

/// 🥓 KBART File harverster
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// File input : a file containing one URL per line
    #[arg(short, long)]
    input: String,
    /// Number of workers
    #[arg(short, long, default_value_t = 5)]
    workers: usize,
    /// Output directory
    #[arg(short, long)]
    output_dir: String,
}

#[derive(Error, Debug)]
enum Errors {
    #[error("The URL must have a path. The last path part is used to name the file (after sanitization)")]
    MissingPath(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let workers = args.workers;
    let input = args.input;
    let output_directory = PathBuf::from(args.output_dir);

    let mut builder = Builder::new();

    builder
        .filter(None, LevelFilter::Info)
        .write_style(WriteStyle::Always)
        .init();

    tokio::fs::create_dir_all(&output_directory).await?;

    let lines = read_lines(&input).await?;
    let stream = LinesStream::new(lines);

    let fetches = stream
        .map(|line| {
            let output_directory = output_directory.clone();
            async move {
                if let Ok(line) = line {
                    if !line.is_empty() {
                        let url: Url = Url::parse(&line)?;
                        let url_path = url
                            .path_segments()
                            .ok_or(Errors::MissingPath(line.clone()))?;

                        let filename = url_path
                            .last()
                            .and_then(|path| if path.is_empty() { None } else { Some(path) })
                            .map(sanitize_filename::sanitize)
                            .map(|filename| output_directory.join(filename))
                            .ok_or(Errors::MissingPath(line.clone()))?;

                        download(&line, filename).await?;
                    }
                }
                Ok(())
            }
        })
        .buffer_unordered(workers)
        .collect::<Vec<Result<(), Box<dyn Error>>>>();

    for elem in fetches.await {
        if let Err(error) = elem { error!("{}", error) }
    }

    Ok(())
}

async fn read_lines(
    filename: &str,
) -> Result<tokio::io::Lines<tokio::io::BufReader<File>>, Box<dyn Error>> {
    let file = File::open(filename).await?;
    Ok(tokio::io::BufReader::new(file).lines())
}

async fn download(url: &str, file_path: PathBuf) -> Result<(), Box<dyn Error>> {
    info!("downloading {}", url);
    let response = reqwest::get(url).await?;
    let mut file = tokio::fs::File::create(file_path).await?;
    let mut content = Cursor::new(response.bytes().await?);
    tokio::io::copy(&mut content, &mut file).await?;
    Ok(())
}
