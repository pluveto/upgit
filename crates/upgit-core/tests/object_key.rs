use upgit_core::ObjectKey;

#[test]
fn parse_strips_leading_and_trailing_slashes() {
    let key = ObjectKey::parse("/2022/01/a.png/").expect("key");
    assert_eq!(key.as_str(), "2022/01/a.png");
}
