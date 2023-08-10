use crate::CtFile;
use anyhow::{bail, Result};
use futures_util::StreamExt;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;

type Shared<T> = Rc<Cell<T>>;

#[derive(Clone)]
pub struct Progress {
    finished: Shared<bool>,
    failed: Shared<bool>,
    fail_message: Rc<RefCell<String>>,

    total: Shared<usize>,
    received: Shared<usize>,
}

impl Progress {
    fn new(total: usize) -> Self {
        let total = Rc::new(Cell::new(total));
        let received = Rc::new(Cell::new(0));
        let finished = Rc::new(Cell::new(false));
        let failed = Rc::new(Cell::new(false));
        let fail_message = Rc::new(RefCell::new("".to_string()));

        Self {
            finished,
            failed,
            fail_message,
            total,
            received,
        }
    }

    pub fn total(&self) -> usize {
        self.total.get()
    }

    fn set_total(&self, total: usize) {
        self.total.set(total);
    }

    pub fn received(&self) -> usize {
        self.received.get()
    }

    fn set_received(&self, received: usize) {
        self.received.set(received);
    }

    pub fn is_finished(&self) -> bool {
        self.finished.get()
    }

    fn finish(&self) {
        self.finished.set(true);
    }

    pub fn is_failed(&self) -> bool {
        self.failed.get()
    }

    pub fn get_err_message(&self) -> Option<String> {
        if self.is_failed() {
            return Some(self.fail_message.clone().take());
        }
        None
    }

    fn fail<T: Into<String>>(&self, message: T) {
        self.failed.set(true);
        *self.fail_message.borrow_mut() = message.into();
    }
}

pub struct DownloadTaskBuilder {
    pub name: String,

    pub progress: Option<Progress>,
    handle: Option<JoinHandle<()>>,
}

pub struct DownloadTask {
    pub name: String,

    pub progress: Progress,
    handle: JoinHandle<()>,
}

impl DownloadTaskBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            progress: None,
            handle: None,
        }
    }

    pub fn set_handle(mut self, handle: JoinHandle<()>) -> Self {
        self.handle = Some(handle);
        self
    }

    pub fn set_progress(mut self, progress: Progress) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn build(self) -> DownloadTask {
        let Self { name, progress, handle } = self;
        let progress = progress.expect("Need to set progress first.");
        let handle = handle.expect("Need to set handle first.");

        DownloadTask { name, progress, handle }
    }
}

async fn download(file: &CtFile, path: &str) -> Result<DownloadTask> {
    let client = reqwest::Client::new();
    let response = client.get(&file.url).send().await?;

    let content_length = response.content_length().map(|x| x as usize).unwrap_or(file.exact_size);
    let progress = Progress::new(content_length);

    let status = response.status();
    if !status.is_success() {
        bail!("unexpected status code: {}, task ends.", status.as_u16());
    }

    let mut stream = response.bytes_stream();
    let mut target = tokio::fs::File::create(path).await?;
    let mut received = 0;

    let progress2 = progress.clone();
    let handle = tokio::task::spawn_local(async move {
        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => {
                    let len = target.write(chunk.as_ref()).await.unwrap();
                    received += len;

                    progress2.set_received(received);
                }
                Err(e) => {
                    progress2.fail(format!("{e}"));
                    return;
                }
            }
        }
        progress2.finish();
    });

    let task = DownloadTaskBuilder::new(path)
        .set_handle(handle)
        .set_progress(progress)
        .build();
    Ok(task)
}

pub struct DownloadQueue {
    queue: Vec<DownloadTask>,
}

impl DownloadQueue {
    pub fn new() -> Self {
        Self { queue: vec![] }
    }

    pub async fn push(&mut self, file: &CtFile) -> Result<()> {
        let task = download(file, ".").await?;
        self.queue.push(task);
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &DownloadTask> {
        self.queue.iter()
    }
}
