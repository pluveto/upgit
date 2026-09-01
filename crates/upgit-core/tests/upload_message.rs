use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use upgit_core::{
    Artifact, KeyPolicy, LinkPolicy, Locator, ObjectKey, Publisher, UploadError, Uploader,
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
