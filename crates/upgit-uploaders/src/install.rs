use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use upgit_core::{KeyPolicy, KeyPolicyError, Registry};

use crate::catalog::RecipeCatalog;
use crate::cos::{CosConfig, CosUploader};
use crate::github::{GithubConfig, GithubUploader};
use crate::gitlab::{GitlabConfig, GitlabUploader};
use crate::oss::{OssConfig, OssUploader};
use crate::qiniu::{QiniuConfig, QiniuUploader};
use crate::recipe::{HttpRecipe, HttpRecipeUploader, RecipeError};
use crate::s3::{S3Config, S3Uploader};
use crate::upyun::{UpyunConfig, UpyunUploader};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse config TOML: {0}")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("uploader `{id}` is missing required field `{field}`")]
    MissingField { id: String, field: String },
    #[error("uploader `{id}` still has a static Qiniu upload token, which expires. Set access_key, secret_key, bucket, and public_base instead.")]
    ExpiredQiniuToken { id: String },
    #[error("unknown uploader type `{kind}` for `{id}`. Built-in types: github, gitlab, s3, aliyunoss, qcloudcos, upyun, qiniu.")]
    UnknownKind { id: String, kind: String },
    #[error("unknown http recipe `{recipe}` for `{id}`")]
    UnknownRecipe { id: String, recipe: String },
    #[error("cannot read recipe `{path}`: {source}")]
    RecipeIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Recipe(#[from] RecipeError),
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(
        default,
        alias = "default_uploader",
        skip_serializing_if = "Option::is_none"
    )]
    pub default: Option<String>,
    #[serde(default, alias = "rename", skip_serializing_if = "Option::is_none")]
    pub naming: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hmac_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hmac_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hmac_len: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_limit: Option<u64>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub output_formats: IndexMap<String, String>,
    #[serde(
        default,
        alias = "replacements",
        skip_serializing_if = "IndexMap::is_empty"
    )]
    pub link: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub uploaders: IndexMap<String, UploaderProfile>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UploaderProfile {
    #[serde(rename = "type", default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<RecipeSpec>,
    #[serde(flatten)]
    pub fields: IndexMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RecipeSpec {
    Name(String),
    Inline(toml::Value),
}

impl AppConfig {
    pub fn from_toml(s: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(s)?)
    }

    pub fn default_uploader(&self) -> Option<&str> {
        self.default.as_deref().filter(|id| !id.is_empty())
    }

    pub fn namer(&self) -> Result<KeyPolicy, KeyPolicyError> {
        const DEFAULT_NAMING: &str = "{year}/{month}/{stem}_{unix}{ext}";
        let template = self
            .naming
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMING);
        if template.contains("{hmac}")
            && self.hmac_key.as_deref().filter(|s| !s.is_empty()).is_none()
        {
            return Err(KeyPolicyError::MissingHmacKey);
        }
        let policy = KeyPolicy::template(template);
        match self.hmac_key.as_deref().filter(|s| !s.is_empty()) {
            Some(key) => Ok(policy.with_hmac(
                key,
                self.hmac_format
                    .as_deref()
                    .unwrap_or("{year}_{month}_{day}_{unix}{ext}"),
                self.hmac_len,
            )),
            None => Ok(policy),
        }
    }

    pub fn linker(&self) -> upgit_core::LinkPolicy {
        upgit_core::LinkPolicy::from_pairs(
            self.link
                .iter()
                .map(|(from, to)| (from.clone(), to.clone())),
        )
    }

    /// Overlay every `UPGIT_*` environment variable onto this config.
    pub fn overlay_env(&mut self) {
        self.apply_env();
    }

    pub fn apply_env(&mut self) {
        self.overlay_from_iter(std::env::vars());
    }

    pub fn overlay_from_iter<I, K, V>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let pairs: Vec<(String, String)> = iter
            .into_iter()
            .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
            .collect();

        let mut value = toml::Value::try_from(&*self)
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
        let mut any = false;
        for (k, v) in &pairs {
            let Some(rest) = k.strip_prefix("UPGIT_") else {
                continue;
            };
            let segments: Vec<String> = rest
                .split("__")
                .filter(|s| !s.is_empty())
                .map(|s| s.to_ascii_lowercase())
                .collect();
            if segments.is_empty() {
                continue;
            }
            insert_path(&mut value, &segments, env_to_value(v));
            any = true;
        }
        if any {
            if let Ok(next) = Self::deserialize(value) {
                *self = next;
            }
        }
        self.apply_legacy_env(&pairs);
    }

    /// 0.2 aliases applied after the Kong-style `UPGIT_` / `__` overlay.
    fn apply_legacy_env(&mut self, pairs: &[(String, String)]) {
        let get = |name: &str| {
            pairs
                .iter()
                .rev()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
        };
        if let Some(pat) = get("UPGIT_TOKEN").or_else(|| get("GITHUB_TOKEN")) {
            self.set_github_field("pat", pat);
        }
        if let Some(username) = get("UPGIT_USERNAME") {
            self.set_github_field("username", username);
        }
        if let Some(repo) = get("UPGIT_REPO") {
            self.set_github_field("repo", repo);
        }
        if let Some(branch) = get("UPGIT_BRANCH") {
            self.set_github_field("branch", branch);
        }
        if let Some(rename) = get("UPGIT_RENAME") {
            self.naming = Some(rename.to_string());
        }
    }

    fn set_github_field(&mut self, key: &str, value: &str) {
        let profile = self.uploaders.entry("github".to_string()).or_default();
        if profile.kind.is_empty() {
            profile.kind = "github".to_string();
        }
        profile
            .fields
            .insert(key.to_string(), toml::Value::String(value.to_string()));
    }

    /// This config object fills a registry with uploader objects.
    pub fn install_into(&self, registry: &mut Registry) -> Result<(), InstallError> {
        for (id, profile) in &self.uploaders {
            match resolved_kind(id, profile)?.as_str() {
                "github" => {
                    let uploader = github_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                "gitlab" => {
                    let uploader = gitlab_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                "s3" => {
                    let uploader = s3_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                "aliyunoss" => {
                    let uploader = oss_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                "qcloudcos" => {
                    let uploader = cos_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                "upyun" => {
                    let uploader = upyun_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                "qiniu" => {
                    let uploader = qiniu_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                "http" => {
                    let uploader = http_from_profile(id, profile)?;
                    registry.register(id.clone(), Box::new(uploader));
                }
                kind => {
                    return Err(InstallError::UnknownKind {
                        id: id.clone(),
                        kind: kind.to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

fn insert_path(root: &mut toml::Value, segments: &[String], val: toml::Value) {
    if segments.is_empty() {
        return;
    }
    if segments.len() == 1 {
        match root.as_table_mut() {
            Some(table) => {
                table.insert(segments[0].clone(), val);
            }
            None => {
                let mut table = toml::map::Map::new();
                table.insert(segments[0].clone(), val);
                *root = toml::Value::Table(table);
            }
        }
        return;
    }
    if !root.is_table() {
        *root = toml::Value::Table(toml::map::Map::new());
    }
    let table = match root {
        toml::Value::Table(table) => table,
        _ => return,
    };
    let entry = table
        .entry(segments[0].clone())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    if !entry.is_table() {
        *entry = toml::Value::Table(toml::map::Map::new());
    }
    insert_path(entry, &segments[1..], val);
}

fn env_to_value(raw: &str) -> toml::Value {
    let trimmed = raw.trim();
    if !trimmed.is_empty()
        && trimmed
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_digit() || (i == 0 && b == b'-'))
    {
        if let Ok(i) = trimmed.parse::<i64>() {
            return toml::Value::Integer(i);
        }
    }
    match trimmed {
        "true" => toml::Value::Boolean(true),
        "false" => toml::Value::Boolean(false),
        _ => toml::Value::String(raw.to_string()),
    }
}

fn resolved_kind(id: &str, profile: &UploaderProfile) -> Result<String, InstallError> {
    let kind = profile.kind.trim();
    if !kind.is_empty() {
        return Ok(kind.to_string());
    }
    let github_keys =
        has_field(profile, "pat") && has_field(profile, "username") && has_field(profile, "repo");
    if id == "github" || github_keys {
        return Ok("github".to_string());
    }
    if id == "gitlab" {
        return Ok("gitlab".to_string());
    }
    // Qiniu uses `bucket` (not bucket_name) and has no endpoint. Check before S3.
    let qiniu_keys = has_field(profile, "access_key")
        && has_field(profile, "secret_key")
        && has_field(profile, "bucket");
    if id == "qiniu" || qiniu_keys {
        return Ok("qiniu".to_string());
    }
    let s3_keys = has_field(profile, "bucket_name")
        && has_field(profile, "endpoint")
        && has_field(profile, "region");
    if id == "s3" || s3_keys {
        return Ok("s3".to_string());
    }
    let oss_keys = has_field(profile, "access_key_id")
        && has_field(profile, "bucket_name")
        && has_field(profile, "endpoint");
    if id == "aliyunoss" || oss_keys {
        return Ok("aliyunoss".to_string());
    }
    let cos_host = optional_string(profile, "host").unwrap_or_default();
    if id == "qcloudcos" || (has_field(profile, "secret_id") && cos_host.contains("cos")) {
        return Ok("qcloudcos".to_string());
    }
    let upyun_keys = has_field(profile, "user_name")
        && has_field(profile, "pass_word")
        && has_field(profile, "bucket_name");
    if id == "upyun" || upyun_keys {
        return Ok("upyun".to_string());
    }
    if profile.recipe.is_some()
        || RecipeCatalog::contains(id)
        || optional_string(profile, "token").is_some()
    {
        return Ok("http".to_string());
    }
    Err(InstallError::UnknownKind {
        id: id.to_string(),
        kind: String::new(),
    })
}

fn has_field(profile: &UploaderProfile, field: &str) -> bool {
    optional_string(profile, field).is_some()
}

fn github_from_profile(
    id: &str,
    profile: &UploaderProfile,
) -> Result<GithubUploader, InstallError> {
    Ok(GithubUploader::new(GithubConfig {
        pat: require_string(id, profile, "pat")?,
        username: require_string(id, profile, "username")?,
        repo: require_string(id, profile, "repo")?,
        branch: optional_string(profile, "branch").unwrap_or_default(),
    }))
}

fn gitlab_from_profile(
    id: &str,
    profile: &UploaderProfile,
) -> Result<GitlabUploader, InstallError> {
    Ok(GitlabUploader::new(GitlabConfig {
        url: optional_string(profile, "url")
            .or_else(|| optional_string(profile, "host"))
            .filter(|s| !is_placeholder(s))
            .ok_or_else(|| InstallError::MissingField {
                id: id.to_string(),
                field: "url".to_string(),
            })?,
        project: require_string(id, profile, "project")?,
        token: require_string(id, profile, "token")?,
        branch: optional_string(profile, "branch").unwrap_or_default(),
        public_base: optional_string(profile, "public_base").filter(|s| !is_placeholder(s)),
    }))
}

fn s3_from_profile(id: &str, profile: &UploaderProfile) -> Result<S3Uploader, InstallError> {
    Ok(S3Uploader::new(S3Config {
        region: require_string(id, profile, "region")?,
        bucket_name: require_string(id, profile, "bucket_name")?,
        access_key: require_string(id, profile, "access_key")?,
        secret_key: require_string(id, profile, "secret_key")?,
        endpoint: require_string(id, profile, "endpoint")?,
        url_format: optional_string(profile, "url_format").unwrap_or_default(),
    }))
}

fn oss_from_profile(id: &str, profile: &UploaderProfile) -> Result<OssUploader, InstallError> {
    Ok(OssUploader::new(OssConfig {
        endpoint: require_string(id, profile, "endpoint")?,
        access_key_id: require_string(id, profile, "access_key_id")?,
        access_key_secret: require_string(id, profile, "access_key_secret")?,
        bucket_name: require_string(id, profile, "bucket_name")?,
        host: require_string(id, profile, "host")?,
    }))
}

fn cos_from_profile(id: &str, profile: &UploaderProfile) -> Result<CosUploader, InstallError> {
    Ok(CosUploader::new(CosConfig {
        host: require_string(id, profile, "host")?,
        secret_id: require_string(id, profile, "secret_id")?,
        secret_key: require_string(id, profile, "secret_key")?,
    }))
}

fn upyun_from_profile(id: &str, profile: &UploaderProfile) -> Result<UpyunUploader, InstallError> {
    let user_name = optional_string(profile, "user_name")
        .or_else(|| optional_string(profile, "username"))
        .filter(|s| !is_placeholder(s))
        .ok_or_else(|| InstallError::MissingField {
            id: id.to_string(),
            field: "user_name".to_string(),
        })?;
    let pass_word = optional_string(profile, "pass_word")
        .or_else(|| optional_string(profile, "password"))
        .filter(|s| !is_placeholder(s))
        .ok_or_else(|| InstallError::MissingField {
            id: id.to_string(),
            field: "pass_word".to_string(),
        })?;
    Ok(UpyunUploader::new(UpyunConfig {
        host: require_string(id, profile, "host")?,
        bucket_name: require_string(id, profile, "bucket_name")?,
        user_name,
        pass_word,
    }))
}

fn qiniu_from_profile(id: &str, profile: &UploaderProfile) -> Result<QiniuUploader, InstallError> {
    let has_ak = optional_string(profile, "access_key").is_some();
    let has_token = optional_string(profile, "token").is_some();
    if has_token && !has_ak {
        return Err(InstallError::ExpiredQiniuToken { id: id.to_string() });
    }
    let public_base = optional_string(profile, "public_base")
        .or_else(|| optional_string(profile, "prefix"))
        .filter(|s| !is_placeholder(s))
        .ok_or_else(|| InstallError::MissingField {
            id: id.to_string(),
            field: "public_base".to_string(),
        })?;
    Ok(QiniuUploader::new(QiniuConfig {
        access_key: require_string(id, profile, "access_key")?,
        secret_key: require_string(id, profile, "secret_key")?,
        bucket: require_string(id, profile, "bucket")?,
        public_base,
        region: optional_string(profile, "region"),
    }))
}

fn http_from_profile(
    id: &str,
    profile: &UploaderProfile,
) -> Result<HttpRecipeUploader, InstallError> {
    let recipe = load_recipe(id, profile)?;
    for key in recipe.required_config_keys() {
        require_string(id, profile, &key)?;
    }
    let mut config = HashMap::new();
    for (key, value) in &profile.fields {
        if let Some(s) = value_as_string(value) {
            config.insert(key.clone(), s);
        }
    }
    Ok(HttpRecipeUploader::new(recipe, config))
}

fn load_recipe(id: &str, profile: &UploaderProfile) -> Result<HttpRecipe, InstallError> {
    match &profile.recipe {
        Some(RecipeSpec::Inline(value)) => {
            let toml_text =
                toml::to_string(value).map_err(|e| RecipeError::Message(e.to_string()))?;
            Ok(HttpRecipe::from_toml(&toml_text)?)
        }
        Some(RecipeSpec::Name(spec)) => recipe_from_name(id, spec),
        None => recipe_from_name(id, id),
    }
}

fn recipe_from_name(id: &str, spec: &str) -> Result<HttpRecipe, InstallError> {
    let path = Path::new(spec);
    if path.is_file() {
        let text = std::fs::read_to_string(path).map_err(|source| InstallError::RecipeIo {
            path: spec.to_string(),
            source,
        })?;
        return Ok(HttpRecipe::from_toml(&text)?);
    }
    if spec.contains('[') {
        return Ok(HttpRecipe::from_toml(spec)?);
    }
    match RecipeCatalog::load(spec) {
        Ok(recipe) => Ok(recipe),
        Err(_) => Err(InstallError::UnknownRecipe {
            id: id.to_string(),
            recipe: spec.to_string(),
        }),
    }
}

fn is_placeholder(s: &str) -> bool {
    let t = s.trim();
    t.is_empty() || t.contains("...") || t.contains("YOUR_") || t.contains("PASTE_")
}

fn require_string(
    id: &str,
    profile: &UploaderProfile,
    field: &str,
) -> Result<String, InstallError> {
    optional_string(profile, field)
        .filter(|s| !is_placeholder(s))
        .ok_or_else(|| InstallError::MissingField {
            id: id.to_string(),
            field: field.to_string(),
        })
}

fn optional_string(profile: &UploaderProfile, field: &str) -> Option<String> {
    profile.fields.get(field).and_then(value_as_string)
}

fn value_as_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Float(f) => Some(f.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}
