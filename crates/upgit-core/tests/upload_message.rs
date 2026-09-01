use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use upgit_core::{
    Artifact, BatchPublisher, KeyPolicy, LinkPolicy, Locator, ObjectKey, Publisher, UploadError,
    Uploader,
};

fn noon() -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_643_630_400)
}

struct RecordingUploader {
    locator: Locator,
    received_key: Mutex<Option<String>>,
    received_name: Mutex<Option<String>>,
}

impl Uploader for RecordingUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        *self.received_key.lock().expect("lock") = Some(key.as_str().to_string());
        *self.received_name.lock().expect("lock") = Some(artifact.file_name().to_string());
        Ok(self.locator.clone())
    }
}

#[test]
fn publisher_asks_uploader_with_the_computed_key() {
    let artifact =
        Artifact::from_name_and_size("logo.png", 2048, Some(5 * 1024 * 1024)).expect("artifact");
    let uploader = RecordingUploader {
        locator: Locator::new("https://cdn.example.com/2022/01/logo_1643630400.png"),
        received_key: Mutex::new(None),
        received_name: Mutex::new(None),
    };
    let publisher = Publisher::new(
        KeyPolicy::template("{year}/{month}/{stem}_{unix}{ext}"),
        LinkPolicy::identity(),
    );
    let url = publisher
        .publish(&uploader, &artifact, noon())
        .expect("publish");

    assert_eq!(
        uploader.received_key.lock().expect("lock").as_deref(),
        Some("2022/01/logo_1643630400.png")
    );
    assert_eq!(
        uploader.received_name.lock().expect("lock").as_deref(),
        Some("logo.png")
    );
    assert_eq!(
        url.as_str(),
        "https://cdn.example.com/2022/01/logo_1643630400.png"
    );
}

struct BatchRecorder {
    received: Mutex<Vec<(String, String)>>,
}

impl Uploader for BatchRecorder {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        self.received
            .lock()
            .expect("lock")
            .push((artifact.file_name().to_string(), key.as_str().to_string()));
        Ok(Locator::new(format!(
            "https://cdn.example.com/{}",
            key.as_str()
        )))
    }
}

fn pngs(n: usize) -> Vec<Artifact> {
    (0..n)
        .map(|i| {
            Artifact::from_name_and_size(&format!("f{i}.png"), 1024, Some(5 * 1024 * 1024))
                .expect("artifact")
        })
        .collect()
}

#[test]
fn batch_publisher_sends_one_upload_per_artifact_and_keeps_order() {
    let artifacts = pngs(12);
    let uploader = BatchRecorder {
        received: Mutex::new(Vec::new()),
    };
    let publisher = Publisher::new(KeyPolicy::keep_original_in("x"), LinkPolicy::identity());
    let urls = BatchPublisher::new(&publisher)
        .with_concurrency(4)
        .run(&uploader, &artifacts, noon())
        .expect("batch");

    assert_eq!(urls.len(), 12);
    for (i, (_raw, url)) in urls.iter().enumerate() {
        assert_eq!(url.as_str(), format!("https://cdn.example.com/x/f{i}.png"));
    }
    let mut received = uploader.received.lock().expect("lock").clone();
    received.sort();
    let mut expected: Vec<_> = (0..12)
        .map(|i| (format!("f{i}.png"), format!("x/f{i}.png")))
        .collect();
    expected.sort();
    assert_eq!(received, expected);
}

struct FailOn {
    name: String,
    seen: Mutex<Vec<String>>,
}

impl Uploader for FailOn {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        self.seen
            .lock()
            .expect("lock")
            .push(artifact.file_name().to_string());
        if artifact.file_name() == self.name {
            return Err(UploadError::message("boom"));
        }
        std::thread::sleep(Duration::from_millis(30));
        Ok(Locator::new(format!("https://ex/{}", key.as_str())))
    }
}

#[test]
fn batch_publisher_defaults_to_serial() {
    let artifacts = pngs(8);
    let uploader = FailOn {
        name: "f0.png".to_string(),
        seen: Mutex::new(Vec::new()),
    };
    let publisher = Publisher::new(KeyPolicy::keep_original_in("x"), LinkPolicy::identity());
    let err = BatchPublisher::new(&publisher)
        .run(&uploader, &artifacts, noon())
        .expect_err("must fail");
    assert!(err.to_string().contains("boom"));
    assert_eq!(
        *uploader.seen.lock().expect("lock"),
        vec!["f0.png".to_string()],
        "default concurrency is 1: stop at the first error without starting the rest"
    );
}

#[test]
fn batch_publisher_fails_the_run_on_first_error() {
    let artifacts = pngs(20);
    let uploader = FailOn {
        name: "f0.png".to_string(),
        seen: Mutex::new(Vec::new()),
    };
    let publisher = Publisher::new(KeyPolicy::keep_original_in("x"), LinkPolicy::identity());
    let err = BatchPublisher::new(&publisher)
        .with_concurrency(4)
        .run(&uploader, &artifacts, noon())
        .expect_err("must fail");
    assert!(err.to_string().contains("boom"));
    let seen = uploader.seen.lock().expect("lock");
    assert!(!seen.is_empty());
    assert!(
        seen.len() < artifacts.len(),
        "must not start every remaining file after a failure, started {}",
        seen.len()
    );
}
