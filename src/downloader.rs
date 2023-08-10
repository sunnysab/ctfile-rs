use crate::CtFile;
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;

#[derive(Clone, Default)]
struct ProgressInner {
    finished: bool,
    failed: bool,
    fail_message: String,

    total: usize,
    received: usize,
}

#[derive(Clone)]
pub struct Progress {
    inner: Rc<RefCell<ProgressInner>>,
}

impl Progress {
    fn new(total: usize) -> Self {
        let progress = ProgressInner {
            total,
            ..ProgressInner::default()
        };

        Self {
            inner: Rc::new(RefCell::new(progress)),
        }
    }

    pub fn total(&self) -> usize {
        self.inner.borrow().total
    }

    fn set_total(&self, total: usize) {
        self.inner.borrow_mut().total = total;
    }

    pub fn received(&self) -> usize {
        self.inner.borrow().received
    }

    fn set_received(&self, received: usize) {
        self.inner.borrow_mut().received = received;
    }

    pub fn is_finished(&self) -> bool {
        self.inner.borrow().finished
    }

    fn finish(&self) {
        self.inner.borrow_mut().finished = true;
    }

    pub fn is_failed(&self) -> bool {
        self.inner.borrow().failed
    }

    pub fn get_err_message(&self) -> Option<String> {
        if self.is_failed() {
            return Some(self.inner.borrow().fail_message.clone());
        }
        None
    }

    fn fail<T: Into<String>>(&self, message: T) {
        self.inner.borrow_mut().failed = true;
        self.inner.borrow_mut().fail_message = message.into();
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

pub async fn download(file: &CtFile, path: &str) -> Result<DownloadTask> {
    let client = reqwest::Client::new();
    let response = client.get(&file.url).send().await?;

    let content_length = response.content_length().map(|x| x as usize).unwrap_or(file.exact_size);
    let progress = Progress::new(content_length);

    let status = response.status();
    if !status.is_success() {
        bail!("unexpected status code: {}, task ends.", status.as_u16());
    }

    let mut stream = response.bytes_stream();

    let mut file_full_path = PathBuf::from_str(path)?;
    file_full_path.push(&file.name);
    let mut target = tokio::fs::File::create(file_full_path).await?;
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
                    progress2.fail(e.to_string());
                    return;
                }
            }
        }
        progress2.finish();
    });

    let task = DownloadTaskBuilder::new(&file.name)
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
        let task = download(file, "/tmp")
            .await
            .context(format!("failed to add {} to download queue.", file.name))?;
        self.queue.push(task);
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &DownloadTask> {
        self.queue.iter()
    }
}
