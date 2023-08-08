use anyhow::{bail, Result};
use futures_util::StreamExt;
use std::cell::Cell;
use std::rc::Rc;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;

use super::CtFileSource;

#[derive(Clone)]
struct Progress {
    finished: Rc<Cell<bool>>,

    total: Rc<Cell<usize>>,
    received: Rc<Cell<usize>>,
}

impl Progress {
    fn new(total: usize) -> Self {
        let total = Rc::new(Cell::new(total));
        let received = Rc::new(Cell::new(0));
        let finished = Rc::new(Cell::new(false));

        Self {
            finished,
            total,
            received,
        }
    }

    fn set_total(&self, total: usize) {
        self.total.set(total);
    }

    fn set_received(&self, received: usize) {
        self.received.set(received);
    }

    fn finished(&self) {
        self.finished.set(true);
    }
}

pub struct DownloadTask {
    name: String,

    progress: Progress,
    handle: Option<JoinHandle<()>>,
}

impl DownloadTask {
    pub fn new(name: &str, total: usize) -> Self {
        Self {
            name: name.to_string(),
            progress: Progress::new(total),
            handle: None,
        }
    }

    pub fn set_handle(&mut self, handle: JoinHandle<()>) {
        self.handle = Some(handle);
    }

    pub fn cancel(&self) {
        if !self.finished.get() {
            if let Some(handle) = &self.handle {
                handle.abort();
            }
        }
    }

    pub fn finish(&self, is_finished: bool) {
        self.finished.set(is_finished);
    }
}

async fn download(url: &str, path: &str, expected_size: usize) -> Result<DownloadTask> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    let content_length = response
        .content_length()
        .map(|x| x as usize)
        .unwrap_or(expected_size);
    let status = response.status();
    if !status.is_success() {
        bail!("unexpected status code: {}, task ends.", status.as_u16());
    }

    let mut progress = Progress::new(content_length);
    let mut stream = response.bytes_stream();
    let mut target = tokio::fs::File::create(path).await?;
    let mut received = 0;

    let handle = tokio::task::spawn_local(async move {
        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => {
                    let len = target.write(chunk.as_ref()).await.unwrap();
                    received += len;

                    progress.set_received(received);
                }
                Err(e) => {}
            }
        }
        progress.finished();
    });

    let mut task = DownloadTask::new(path, content_length);
    task.handle = Some(handle);
    Ok(task)
}

pub struct DownloadQueue {
    queue: Vec<DownloadTask>,
}

impl DownloadQueue {
    async fn push(&mut self, source: &CtFileSource) -> Result<()> {
        let url = &source.url;
        let filename = &source.name;

        let task = download(url, filename, source.exact_size).await?;
        self.queue.push(task);
        Ok(())
    }
}
