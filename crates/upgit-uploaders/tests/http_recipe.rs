//! Unsigned HTTP image hosts are recipes: interpolate a request, then pull a URL
//! from a JSON path or a template. Recipes do not mint signatures.

use upgit_uploaders::recipe::{HttpRecipe, RecipeContext};

fn smms_recipe() -> HttpRecipe {
    HttpRecipe::from_toml(
        r#"
[meta]
id = "smms"

[request]
method = "POST"
url = "https://sm.ms/api/v2/upload"

[request.headers]
Authorization = "{config.token}"

[request.body]
format = { type = "string", value = "json" }
smfile = { type = "file" }

[response]
url = { from = "json", path = "data.url" }
"#,
    )
    .expect("parse recipe")
}

#[test]
fn extracts_url_from_json_path() {
    let recipe = smms_recipe();
    let body = br#"{"success":true,"data":{"url":"https://i.loli.net/2022/01/a.png"}}"#;
    let ctx = RecipeContext::default();
    let locator = recipe.extract_locator(body, &ctx).expect("url");
    assert_eq!(locator.as_str(), "https://i.loli.net/2022/01/a.png");
}

#[test]
fn missing_json_path_fails() {
    let recipe = smms_recipe();
    let body = br#"{"success":true,"data":{"hash":"abc"}}"#;
    let err = recipe
        .extract_locator(body, &RecipeContext::default())
        .expect_err("missing path");
    let msg = err.to_string();
    assert!(
        msg.contains("data.url") || msg.contains("path") || msg.contains("json"),
        "got {msg}"
    );
}

#[test]
fn wrong_json_is_not_silently_empty() {
    let recipe = smms_recipe();
    let err = recipe
        .extract_locator(br#"not-json"#, &RecipeContext::default())
        .expect_err("invalid json");
    assert!(!err.to_string().is_empty());
}

#[test]
fn template_response_builds_locator_from_context() {
    let recipe = HttpRecipe::from_toml(
        r#"
[meta]
id = "qiniu-shaped-but-unsigned"

[request]
method = "POST"
url = "https://example.invalid/upload"

[response]
url = { from = "template", template = "{config.public_base}{key}" }
"#,
    )
    .expect("parse");
    let mut ctx = RecipeContext::new();
    ctx.put("config.public_base", "https://cdn.example.com/");
    ctx.put("key", "2022/01/a.png");
    let locator = recipe.extract_locator(b"", &ctx).expect("url");
    assert_eq!(locator.as_str(), "https://cdn.example.com/2022/01/a.png");
}

#[test]
fn header_placeholder_is_plain_string_interpolation() {
    let recipe = smms_recipe();
    let mut ctx = RecipeContext::new();
    ctx.put("config.token", "smms_secret");
    let headers = recipe.interpolated_headers(&ctx).expect("headers");
    assert_eq!(
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .map(|(_, v)| v.as_str()),
        Some("smms_secret")
    );
}
