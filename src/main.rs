pub mod api;
pub mod downloader;

use anyhow::Result;
use clap::Parser;
use tokio::task;

pub use api::{get_file_by_id, get_file_by_link};
pub use api::{CtFile, CtFileSource};
pub use downloader::DownloadQueue;

const DEFAULT_TOKEN: &str = "5sijtqc2rlocvvkvmn7777";
const DEFAULT_LISTEN_ADDR: &str = "localhost:7735";

#[derive(clap::Parser)]
#[command(name = "ctfile-rs")]
#[command(author = "sunnysab <i@sunnysab.cn>")]
#[command(version = "0.1")]
#[command(about = "Download file from ctfile.com via CLI.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Daemon {
        #[arg(short, long)]
        listen: Option<String>,
    },
    Parse {
        url: String,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(short, long)]
        token: Option<String>,
    },
    Download {
        url: String,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(short, long)]
        token: Option<String>,
        #[arg(short, long)]
        daemon: Option<String>,
    },
    List {
        #[arg(short, long)]
        daemon: Option<String>,
    },
}

fn display_file_size(len: usize) -> String {
    let mut n: usize = 1024 * 1024 * 1024;
    let mut r = len / n;
    let t = ["GB", "MB", "KB", "Byte"];

    if len == 0 {
        return String::new();
    }
    let mut i: usize = 0;
    while r == 0 {
        n /= 1024;
        r = len / n;
        i += 1;
    }
    t[i].to_string()
}

async fn do_link_parsing(
    url: &str,
    password: Option<String>,
    token: Option<String>,
) -> Result<(CtFile, CtFileSource)> {
    println!("URL {}", url);
    println!("Fetching file metadata...");
    let token = token.unwrap_or(DEFAULT_TOKEN.to_string());
    let file = get_file_by_link(&url, password, &token).await?;

    println!(
        "File {} ({}) uploaded on {}\nChecksum {}",
        file.file_name, file.file_id, file.file_time, file.file_chk
    );
    let source = file.get_download_source().await?;
    println!(
        "Length: {} ({})",
        source.exact_size,
        display_file_size(source.exact_size)
    );
    println!("Parsed link: {}", source.url);
    Ok((file, source))
}

async fn serve(cli: Cli) -> Result<()> {
    let mut queue = DownloadQueue::new();

    match cli.command {
        Commands::Daemon { .. } => {}
        Commands::Parse {
            url,
            password,
            token,
        } => {
            let _ = do_link_parsing(&url, password, token).await?;
        }
        Commands::Download {
            url,
            password,
            token,
            daemon,
        } => {
            let (_file, source) = do_link_parsing(&url, password, token).await?;
            if let Some(_daemon) = daemon {
                // TODO: 通知 daemon 添加任务.
            } else {
                // TODO: 传入 checksum 供校验.
                queue
                    .push(&source)
                    .await
                    .expect("failed to add item to download queue.");
            }
        }
        Commands::List { daemon } => {
            unimplemented!()
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    let local = task::LocalSet::new();
    local
        .run_until(serve(cli))
        .await
        .expect("Unable to complete command...")
}
