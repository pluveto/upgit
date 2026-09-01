//! Unsigned HTTP image hosts are recipes: interpolate a request, then pull a URL
//! from a JSON path or a template. Recipes do not mint signatures.

use std::collections::HashMap;

use upgit_uploaders::recipe::{HttpRecipe, HttpRecipeUploader, RecipeContext, RecipeError};
use upgit_uploaders::RecipeCatalog;

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
fn text_response_is_the_locator() {
    let recipe = HttpRecipe::from_toml(
        r#"
[meta]
id = "catbox"
[request]
method = "POST"
url = "https://example.invalid/"
[response]
url = { from = "text" }
"#,
    )
    .expect("parse");
    let locator = recipe
        .extract_locator(b"https://files.catbox.moe/abc.png\n", &RecipeContext::new())
        .expect("url");
    assert_eq!(locator.as_str(), "https://files.catbox.moe/abc.png");
}

#[test]
fn missing_config_placeholder_is_not_empty_string() {
    let err = RecipeContext::new()
        .interpolate("Bearer {config.token}")
        .expect_err("missing config");
    match err {
        RecipeError::MissingPlaceholder(key) => assert_eq!(key, "config.token"),
        other => panic!("expected MissingPlaceholder, got {other}"),
    }
}

#[test]
fn required_config_keys_are_unique_and_stable() {
    let recipe = smms_recipe();
    assert_eq!(recipe.required_config_keys(), vec!["token".to_string()]);
    let gitee = HttpRecipe::from_toml(
        r#"
[meta]
id = "gitee"
[request]
method = "POST"
url = "https://gitee.com/api/v5/repos/{config.username}/{config.repo}/contents/{key}"
[request.body]
access_token = { type = "string", value = "{config.access_token}" }
content = { type = "file_base64" }
[response]
url = { from = "template", template = "https://gitee.com/{config.username}/{config.repo}/raw/{key}" }
"#,
    )
    .expect("parse");
    assert_eq!(
        gitee.required_config_keys(),
        vec![
            "username".to_string(),
            "repo".to_string(),
            "access_token".to_string()
        ]
    );
}

#[test]
fn bundled_catbox_recipe_does_not_require_userhash() {
    let recipe = RecipeCatalog::load("catbox").expect("catbox");
    assert_eq!(recipe.required_config_keys(), Vec::<String>::new());
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

fn smms_uploader() -> HttpRecipeUploader {
    let mut config = HashMap::new();
    config.insert("token".into(), "secret".into());
    HttpRecipeUploader::new(smms_recipe(), config)
}

#[test]
fn explain_401_points_at_config_keys_not_json() {
    let err = smms_uploader().explain(
        401,
        r#"{"success":false,"message":"unauthorized","code":"unauthorized"}"#,
    );
    let s = err.to_string();
    assert!(s.contains("401"), "got {s}");
    assert!(
        s.contains("token") || s.contains("uploaders.smms"),
        "got {s}"
    );
    assert!(!s.contains("\"success\""), "dumped JSON: {s}");
}

#[test]
fn explain_404_does_not_dump_body() {
    let err = smms_uploader().explain(
        404,
        r#"{"message":"Not Found","documentation_url":"https://example"}"#,
    );
    let s = err.to_string();
    assert!(s.contains("not found") || s.contains("404"), "got {s}");
    assert!(!s.contains("documentation_url"), "dumped JSON: {s}");
}
