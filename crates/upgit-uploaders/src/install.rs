use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::Deserialize;
use thiserror::Error;
use upgit_core::Registry;

use crate::qiniu::{QiniuConfig, QiniuUploader};
use crate::recipe::{HttpRecipe, HttpRecipeUploader, RecipeError};

const SMMS_RECIPE: &str = r#"
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
"#;

const IMGUR_RECIPE: &str = r#"
[meta]
id = "imgur"

[request]
method = "POST"
url = "https://api.imgur.com/3/upload"

[request.headers]
Authorization = "Client-ID {config.client_id}"

[request.body]
image = { type = "file" }

[response]
url = { from = "json", path = "data.link" }
"#;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse config TOML: {0}")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("uploader `{id}` is missing required field `{field}`")]
    MissingField { id: String, field: String },
    #[error("uploader `{id}` still has a static Qiniu upload token, which expires. Set access_key, secret_key, bucket, and public_base (alias: prefix) instead. Run `upgit init` for a sample. Do not use an extensions/*.jsonc file.")]
    ExpiredQiniuToken { id: String },
    #[error("unknown uploader type `{kind}` for `{id}` (qiniu is built-in: access_key, secret_key, bucket, public_base; HTTP hosts use type = \"http\"). There is no extensions/ directory.")]
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default, alias = "default_uploader")]
    pub default: Option<String>,
    #[serde(default, alias = "rename")]
    pub naming: Option<String>,
    #[serde(default)]
    pub hmac_key: Option<String>,
    #[serde(default)]
    pub hmac_format: Option<String>,
    #[serde(default)]
    pub hmac_len: Option<usize>,
    #[serde(default, alias = "replacements")]
    pub link: IndexMap<String, String>,
    #[serde(default)]
    pub uploaders: IndexMap<String, UploaderProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploaderProfile {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub recipe: Option<RecipeSpec>,
    #[serde(flatten)]
    pub fields: IndexMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize)]
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

    pub fn namer(&self) -> upgit_core::KeyPolicy {
        use upgit_core::KeyPolicy;
        const DEFAULT_NAMING: &str = "{year}/{month}/{stem}_{unix}{ext}";
        let template = self
            .naming
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMING);
        let policy = KeyPolicy::template(template);
        match self.hmac_key.as_deref().filter(|s| !s.is_empty()) {
            Some(key) => policy.with_hmac(
                key,
                self.hmac_format
                    .as_deref()
                    .unwrap_or("{year}_{month}_{day}_{unix}{ext}"),
                self.hmac_len,
            ),
            None => policy,
        }
    }

    pub fn linker(&self) -> upgit_core::LinkPolicy {
        upgit_core::LinkPolicy::from_pairs(
            self.link
                .iter()
                .map(|(from, to)| (from.clone(), to.clone())),
        )
    }

    /// This config object fills a registry with uploader objects.
    pub fn install_into(&self, registry: &mut Registry) -> Result<(), InstallError> {
        for (id, profile) in &self.uploaders {
            match resolved_kind(id, profile)?.as_str() {
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

fn resolved_kind(id: &str, profile: &UploaderProfile) -> Result<String, InstallError> {
    let kind = profile.kind.trim();
    if !kind.is_empty() {
        return Ok(kind.to_string());
    }
    let qiniu_keys = optional_string(profile, "access_key").is_some()
        && optional_string(profile, "secret_key").is_some()
        && optional_string(profile, "bucket").is_some();
    if id == "qiniu" || qiniu_keys {
        return Ok("qiniu".to_string());
    }
    if profile.recipe.is_some() || optional_string(profile, "token").is_some() {
        return Ok("http".to_string());
    }
    Err(InstallError::UnknownKind {
        id: id.to_string(),
        kind: String::new(),
    })
}

fn qiniu_from_profile(id: &str, profile: &UploaderProfile) -> Result<QiniuUploader, InstallError> {
    let has_ak = optional_string(profile, "access_key").is_some();
    let has_token = optional_string(profile, "token").is_some();
    if has_token && !has_ak {
        return Err(InstallError::ExpiredQiniuToken { id: id.to_string() });
    }
    let public_base = optional_string(profile, "public_base")
        .or_else(|| optional_string(profile, "prefix"))
        .filter(|s| !s.is_empty())
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
    if let Some(embedded) = embedded_recipe(spec) {
        return Ok(HttpRecipe::from_toml(embedded)?);
    }
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
    Err(InstallError::UnknownRecipe {
        id: id.to_string(),
        recipe: spec.to_string(),
    })
}

fn embedded_recipe(id: &str) -> Option<&'static str> {
    match id {
        "smms" => Some(SMMS_RECIPE),
        "imgur" => Some(IMGUR_RECIPE),
        _ => None,
    }
}

fn require_string(
    id: &str,
    profile: &UploaderProfile,
    field: &str,
) -> Result<String, InstallError> {
    optional_string(profile, field)
        .filter(|s| !s.is_empty())
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
