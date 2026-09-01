use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::SystemTime;

use thiserror::Error;

use crate::artifact::Artifact;
use crate::key_policy::{KeyPolicy, KeyPolicyError};
use crate::link_policy::LinkPolicy;
use crate::locator::{Locator, PublicUrl};
use crate::uploader::{UploadError, Uploader};

#[derive(Debug, Error)]
pub enum PublishError {
    #[error(transparent)]
    Key(#[from] KeyPolicyError),
    #[error(transparent)]
    Upload(#[from] UploadError),
}

/// Orchestrates naming, upload, and link rewrite. Holds its collaborators;
/// the uploader is passed in as the recipient of the upload message.
pub struct Publisher {
    namer: KeyPolicy,
    linker: LinkPolicy,
}

/// At most this many `upload` messages are in flight. 183 files must not spawn 183 threads.
const DEFAULT_UPLOAD_CONCURRENCY: usize = 8;

type Slot = Mutex<Option<Result<(Locator, PublicUrl), PublishError>>>;

/// Sends one `upload` message per artifact, with a bound on in-flight work.
///
/// Results stay in input order. After the first failure, further artifacts are
/// not started; in-flight messages still finish. The error is the failed
/// artifact with the lowest input index.
pub struct BatchPublisher<'a> {
    publisher: &'a Publisher,
    concurrency: usize,
}

impl Publisher {
    pub fn new(namer: KeyPolicy, linker: LinkPolicy) -> Self {
        Self { namer, linker }
    }

    pub fn publish(
        &self,
        uploader: &dyn Uploader,
        artifact: &Artifact,
        at: SystemTime,
    ) -> Result<PublicUrl, PublishError> {
        Ok(self.publish_with_raw(uploader, artifact, at)?.1)
    }

    /// Locator before `[link]` replacements, and the rewritten public URL.
    pub fn publish_with_raw(
        &self,
        uploader: &dyn Uploader,
        artifact: &Artifact,
        at: SystemTime,
    ) -> Result<(Locator, PublicUrl), PublishError> {
        let key = self.namer.apply(artifact, at)?;
        let locator = uploader.upload(artifact, &key)?;
        let url = self.linker.apply(&locator);
        Ok((locator, url))
    }
}

impl<'a> BatchPublisher<'a> {
    pub fn new(publisher: &'a Publisher) -> Self {
        Self {
            publisher,
            concurrency: DEFAULT_UPLOAD_CONCURRENCY,
        }
    }

    pub fn with_concurrency(self, concurrency: usize) -> Self {
        Self {
            concurrency: concurrency.max(1),
            ..self
        }
    }

    pub fn run(
        &self,
        uploader: &dyn Uploader,
        artifacts: &[Artifact],
        at: SystemTime,
    ) -> Result<Vec<(Locator, PublicUrl)>, PublishError> {
        match artifacts {
            [] => return Ok(Vec::new()),
            [one] => return Ok(vec![self.publisher.publish_with_raw(uploader, one, at)?]),
            _ => {}
        }

        let n = artifacts.len();
        let workers = self.concurrency.min(n);
        let next = AtomicUsize::new(0);
        let failed = AtomicBool::new(false);
        let slots: Vec<Slot> = (0..n).map(|_| Mutex::new(None)).collect();

        thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    if failed.load(Ordering::Relaxed) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    if failed.load(Ordering::Relaxed) {
                        break;
                    }
                    let result = self.publisher.publish_with_raw(uploader, &artifacts[i], at);
                    if result.is_err() {
                        failed.store(true, Ordering::Relaxed);
                    }
                    *slots[i].lock().expect("slot") = Some(result);
                });
            }
        });

        collect_slots(slots)
    }
}

fn collect_slots(slots: Vec<Slot>) -> Result<Vec<(Locator, PublicUrl)>, PublishError> {
    let n = slots.len();
    let mut out = Vec::with_capacity(n);
    let mut first_err = None;
    let mut missing = false;
    for slot in slots {
        match slot.into_inner().expect("slot") {
            Some(Ok(value)) => {
                if first_err.is_none() {
                    out.push(value);
                }
            }
            Some(Err(err)) => {
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
            None => missing = true,
        }
    }
    if let Some(err) = first_err {
        return Err(err);
    }
    if missing {
        return Err(UploadError::message("upload did not finish").into());
    }
    Ok(out)
}
