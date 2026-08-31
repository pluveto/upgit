use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;
use upgit_core::{Artifact, Locator, ObjectKey, UploadError, Uploader};

use crate::form::{self, Part};

#[derive(Debug, Error)]
pub enum RecipeError {
    #[error("failed to parse recipe TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid json response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("json path `{path}` not found")]
    MissingPath { path: String },
    #[error("missing placeholder `{0}`")]
    MissingPlaceholder(String),
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone, Default)]
pub struct RecipeContext {
    values: HashMap<String, String>,
}

impl RecipeContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(&mut self, key: &str, value: impl Into<String>) {
        self.values.insert(key.to_string(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn interpolate(&self, template: &str) -> Result<String, RecipeError> {
        let mut out = String::new();
        let mut rest = template;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            rest = &rest[start + 1..];
            match rest.find('}') {
                Some(end) => {
                    let key = &rest[..end];
                    let value = self
                        .get(key)
                        .ok_or_else(|| RecipeError::MissingPlaceholder(key.to_string()))?;
                    out.push_str(value);
                    rest = &rest[end + 1..];
                }
                None => {
                    out.push('{');
                    out.push_str(rest);
                    rest = "";
                }
            }
        }
        out.push_str(rest);
        Ok(out)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HttpRecipe {
    meta: Meta,
    request: Request,
    response: Response,
}

#[derive(Debug, Clone, Deserialize)]
struct Meta {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Request {
    method: String,
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: HashMap<String, BodyField>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum BodyField {
    #[serde(rename = "string")]
    String { value: String },
    #[serde(rename = "file")]
    File {},
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
    url: UrlSpec,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "from")]
enum UrlSpec {
    #[serde(rename = "json")]
    Json { path: String },
    #[serde(rename = "template")]
    Template { template: String },
}

impl HttpRecipe {
    pub fn from_toml(s: &str) -> Result<Self, RecipeError> {
        Ok(toml::from_str(s)?)
    }

    pub fn id(&self) -> &str {
        &self.meta.id
    }

    pub fn extract_locator(
        &self,
        body: &[u8],
        ctx: &RecipeContext,
    ) -> Result<Locator, RecipeError> {
        match &self.response.url {
            UrlSpec::Json { path } => {
                let value: serde_json::Value = serde_json::from_slice(body)?;
                Ok(Locator::new(Self::lookup_path(&value, path)?))
            }
            UrlSpec::Template { template } => Ok(Locator::new(ctx.interpolate(template)?)),
        }
    }

    pub fn interpolated_headers(
        &self,
        ctx: &RecipeContext,
    ) -> Result<Vec<(String, String)>, RecipeError> {
        self.request
            .headers
            .iter()
            .map(|(k, v)| Ok((k.clone(), ctx.interpolate(v)?)))
            .collect()
    }

    fn lookup_path(value: &serde_json::Value, path: &str) -> Result<String, RecipeError> {
        let mut cur = value;
        for part in path.split('.') {
            cur = cur.get(part).ok_or_else(|| RecipeError::MissingPath {
                path: path.to_string(),
            })?;
        }
        match cur.as_str() {
            Some(s) => Ok(s.to_string()),
            None => Err(RecipeError::MissingPath {
                path: path.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpRecipeUploader {
    recipe: HttpRecipe,
    config: HashMap<String, String>,
}

impl HttpRecipeUploader {
    pub fn new(recipe: HttpRecipe, config: HashMap<String, String>) -> Self {
        Self { recipe, config }
    }

    fn context(&self, key: &ObjectKey) -> RecipeContext {
        let mut ctx = RecipeContext::new();
        ctx.put("key", key.as_str());
        for (k, v) in &self.config {
            ctx.put(&format!("config.{k}"), v);
        }
        ctx
    }

    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        parts: &[Part<'_>],
    ) -> Result<Vec<u8>, UploadError> {
        let mut request = ureq::request(&self.recipe.request.method, url);
        for (k, v) in headers {
            if k.eq_ignore_ascii_case("content-type") {
                continue;
            }
            request = request.set(k, v);
        }
        if parts.is_empty() {
            Self::read_response(request.call())
        } else {
            let (content_type, body) = form::encode(parts);
            Self::read_response(request.set("Content-Type", &content_type).send_bytes(&body))
        }
    }

    fn read_response(result: Result<ureq::Response, ureq::Error>) -> Result<Vec<u8>, UploadError> {
        match result {
            Ok(resp) => resp
                .into_string()
                .map(|s| s.into_bytes())
                .map_err(|e| UploadError::message(e.to_string())),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(UploadError::message(format!("upload HTTP {code}: {text}")))
            }
            Err(e) => Err(UploadError::message(e.to_string())),
        }
    }
}

impl Uploader for HttpRecipeUploader {
    fn upload(&self, artifact: &Artifact, key: &ObjectKey) -> Result<Locator, UploadError> {
        let ctx = self.context(key);
        let url = ctx
            .interpolate(&self.recipe.request.url)
            .map_err(upload_err)?;
        let headers = self.recipe.interpolated_headers(&ctx).map_err(upload_err)?;

        let file_bytes = if self
            .recipe
            .request
            .body
            .values()
            .any(|f| matches!(f, BodyField::File { .. }))
        {
            let path = artifact.path().ok_or_else(|| {
                UploadError::message("artifact has no local path; cannot upload bytes")
            })?;
            Some(std::fs::read(path).map_err(|e| UploadError::message(e.to_string()))?)
        } else {
            None
        };

        let mut text_values = Vec::new();
        for (name, field) in &self.recipe.request.body {
            if let BodyField::String { value } = field {
                text_values.push((name.clone(), ctx.interpolate(value).map_err(upload_err)?));
            }
        }

        let mut parts = Vec::new();
        for (name, value) in &text_values {
            parts.push(Part::Text { name, value });
        }
        if let Some(data) = file_bytes.as_deref() {
            for (name, field) in &self.recipe.request.body {
                if matches!(field, BodyField::File { .. }) {
                    parts.push(Part::File {
                        name,
                        filename: artifact.file_name(),
                        data,
                    });
                }
            }
        }

        let response_body = self.post(&url, &headers, &parts)?;
        self.recipe
            .extract_locator(&response_body, &ctx)
            .map_err(upload_err)
    }
}

fn upload_err(e: RecipeError) -> UploadError {
    UploadError::message(e.to_string())
}
