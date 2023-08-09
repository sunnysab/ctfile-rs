pub mod api;
pub mod downloader;

use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;
use tokio::task;

pub use api::{get_file_by_id, CtFile, CtFileSource};
pub use downloader::DownloadQueue;

pub const DEFAULT_TOKEN: &str = "5sijtqc2rlocvvkvmn7777";

async fn serve() -> Result<()> {
    let mut queue = DownloadQueue::new();

    let file = api::get_file_by_id("4070316-134836896", "", DEFAULT_TOKEN).await?;
    let source = file.get_download_source().await?;
    println!("{file:#?}");
    println!("source = {source:#?}");

    queue
        .push(&source)
        .await
        .expect("failed to add item to download queue.");

    loop {
        for item in queue.iter() {
            let progress = &item.progress;

            println!(
                "[{}] {} bytes downloaded, {} bytes in total.",
                item.name,
                progress.received(),
                progress.total()
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let local = task::LocalSet::new();

    local.run_until(serve()).await.unwrap()
}
