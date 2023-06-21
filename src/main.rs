use clap::Parser;
use env_logger::{Builder, WriteStyle};
use futures::StreamExt;
use log::{error, info, LevelFilter};
use reqwest::header::HeaderMap;
use reqwest::{Client, Url};
use std::error::Error;
use std::io::Cursor;
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;

// Selon les version de KBART il y a deux types de header possible
const KBART_HEADER : &'static str = "publication_title	print_identifier	online_identifier	date_first_issue_online	num_first_vol_online	num_first_issue_online	date_last_issue_online	num_last_vol_online	num_last_issue_online	title_url	first_author	title_id	embargo_info	coverage_depth	notes	publisher_name	publication_type";
const KBART_HEADER_5321 : &'static str = "publication_title	print_identifier	online_identifier	date_first_issue_online	num_first_vol_online	num_first_issue_online	date_last_issue_online	num_last_vol_online	num_last_issue_online	title_url	first_author	title_id	embargo_info	coverage_depth	coverage_notes	publisher_name	publication_type";


/// 🥓 KBART File harverster
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// File input : a file containing one URL per line. If not set urls are read from STDIN.
    #[arg(short, long)]
    input: Option<String>,
    /// Number of workers
    #[arg(short, long, default_value_t = 5)]
    workers: usize,
    /// Output directory
    #[arg(short, long)]
    output_dir: String,
    /// Dont check kbart file validity
    #[arg(short,long, default_value_t = false)]
    nocheck: bool
}

#[derive(Error, Debug)]
enum Errors {
    #[error("The URL must have a path. The last path part is used to name the file (after sanitization)")]
    MissingPath(String),
    #[error("The kbart file must have a valid header")]
    InvalidKbartFile(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let workers = args.workers;
    let output_directory = PathBuf::from(args.output_dir);
    let check_validity = !args.nocheck;

    let mut builder = Builder::new();

    builder
        .filter(None, LevelFilter::Info)
        .write_style(WriteStyle::Always)
        .init();

    tokio::fs::create_dir_all(&output_directory).await?;

    match args.input {
        None => {
            info!("reading data from stdin");
            let stdin = tokio::io::stdin();
            let reader = tokio::io::BufReader::new(stdin);
            process(LinesStream::new(reader.lines()), output_directory, workers, check_validity).await;
        }
        Some(file) => {
            let lines = read_lines(&file).await?;
            process(LinesStream::new(lines), output_directory, workers, check_validity).await;
        }
    }

    Ok(())
}

async fn read_lines(
    filename: &str,
) -> Result<tokio::io::Lines<tokio::io::BufReader<File>>, Box<dyn Error>> {
    let file = File::open(filename).await?;
    Ok(tokio::io::BufReader::new(file).lines())
}

async fn check_header(url: &str) -> Result<(), Box<dyn Error>> {
    info!("checking kbart header of {}", url);
    let mut headers = HeaderMap::new();
    // * 2 certains providers fournissent de l'UTF-16
    headers.append(
        "Range",
        format!("bytes=0-{}", KBART_HEADER_5321.bytes().count() * 2).parse()?,
    );

    headers.append("Accept-Charset", "utf-8".parse()?);

    let request = Client::new().get(url).headers(headers).build()?;

    let response = Client::new().execute(request).await?.text().await?;

    // Si le serveur ne supporte pas le byte range il retourne l'intégralité du document.
    // On vérifie donc que le header est présent avec starts_with et non avec une égalité parfaite.
    if response.starts_with(KBART_HEADER_5321) || response.starts_with(KBART_HEADER)  {
        Ok(())
    } else {
        error!("{} has an invalid kbart header", url);
        Err(Errors::InvalidKbartFile(url.to_string()).into())
    }
}

async fn download(url: &str, file_path: PathBuf, check_file: bool) -> Result<(), Box<dyn Error>> {
    if check_file {
        check_header(url).await?;
    }

    info!("downloading {}", url);
    let response = reqwest::get(url).await?;
    let mut file = tokio::fs::File::create(file_path).await?;
    let mut content = Cursor::new(response.bytes().await?);
    tokio::io::copy(&mut content, &mut file).await?;
    Ok(())
}

async fn process<T: tokio_stream::Stream<Item = Result<String, std::io::Error>>>(
    stream: T,
    output_directory: PathBuf,
    workers: usize,
    check_validity: bool
) -> () {
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

                    download(&line, filename, check_validity).await?;
                }
            }
            Ok(())
        }
    })
    .buffer_unordered(workers)
    .collect::<Vec<Result<(), Box<dyn Error>>>>();

for elem in fetches.await {
    if let Err(error) = elem {
        error!("{}", error)
    }
}
}
