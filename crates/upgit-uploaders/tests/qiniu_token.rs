use std::time::{Duration, UNIX_EPOCH};

use upgit_uploaders::qiniu::QiniuUploader;

const DEADLINE: u64 = 1_643_630_400;

/// Independent Python HMAC-SHA1 + urlsafe-b64 vector (not copied from the impl).
const EXPECTED_TOKEN: &str = "test_ak:92T_VYZYdbzmcAItdA_Xlgh8MVc=:eyJzY29wZSI6InRlc3QtYnVja2V0IiwiZGVhZGxpbmUiOjE2NDM2MzA0MDB9";

#[test]
fn mint_token_matches_independent_vector() {
    let token = QiniuUploader::mint_token(
        "test_ak",
        "test_sk",
        "test-bucket",
        UNIX_EPOCH + Duration::from_secs(DEADLINE),
    );
    assert_eq!(token, EXPECTED_TOKEN);
}
