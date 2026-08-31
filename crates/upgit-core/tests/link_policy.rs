//! LinkPolicy rewrites a Locator into a PublicUrl after upload.
//! It is not the Uploader's job (GitHub jsDelivr vs Qiniu public_base are different).

use upgit_core::{LinkPolicy, Locator};

#[test]
fn identity_leaves_locator_unchanged() {
    let policy = LinkPolicy::identity();
    let locator = Locator::new("https://cdn.example.com/2022/01/a.png");
    assert_eq!(
        policy.apply(&locator).as_str(),
        "https://cdn.example.com/2022/01/a.png"
    );
}

#[test]
fn github_raw_to_jsdelivr_style_pairs() {
    let policy = LinkPolicy::from_pairs([
        (
            "raw.githubusercontent.com".to_string(),
            "cdn.jsdelivr.net/gh".to_string(),
        ),
        ("/master".to_string(), "@master".to_string()),
    ]);
    let locator = Locator::new("https://raw.githubusercontent.com/user/repo/master/2022/01/a.png");
    assert_eq!(
        policy.apply(&locator).as_str(),
        "https://cdn.jsdelivr.net/gh/user/repo@master/2022/01/a.png"
    );
}

#[test]
fn no_pairs_means_identity() {
    let policy = LinkPolicy::from_pairs(Vec::<(String, String)>::new());
    let locator = Locator::new("https://i.loli.net/a.png");
    assert_eq!(policy.apply(&locator).as_str(), "https://i.loli.net/a.png");
}
