use std::error::Error;

use upgit::Output;

use crate::source::explain_clipboard;

enum Format {
    Url,
    Markdown,
    Template(String),
}

pub struct Emitter {
    dest: Output,
    format: Format,
}

impl Emitter {
    pub fn new(
        dest: Output,
        format: Option<&str>,
        configured: &[(String, String)],
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            dest,
            format: resolve_format(format.unwrap_or("url"), configured)?,
        })
    }

    pub fn send(&self, urls: &[String]) -> Result<(), Box<dyn Error>> {
        let text = urls
            .iter()
            .map(|url| self.format_url(url))
            .collect::<Vec<_>>()
            .join("\n");
        match self.dest {
            Output::Clipboard => {
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| explain_clipboard("clipboard is unavailable", e))?;
                clipboard.set_text(&text).map_err(|e| {
                    let mut msg = explain_clipboard("cannot copy URL to clipboard", e);
                    if cfg!(target_os = "linux")
                        && !msg.contains("xclip")
                        && !msg.contains("wl-clipboard")
                    {
                        msg.push_str("\nLinux needs `xclip` (X11) or `wl-clipboard` (Wayland).");
                    }
                    msg
                })?;
            }
            Output::Stdout => println!("{text}"),
        }
        Ok(())
    }

    fn format_url(&self, url: &str) -> String {
        let fname = url_fname(url);
        match &self.format {
            Format::Url => url.to_string(),
            Format::Markdown => format!("![{fname}]({url})"),
            Format::Template(template) => {
                template.replace("{url_fname}", fname).replace("{url}", url)
            }
        }
    }
}

fn resolve_format(name: &str, configured: &[(String, String)]) -> Result<Format, Box<dyn Error>> {
    match name {
        "url" => Ok(Format::Url),
        "markdown" => Ok(Format::Markdown),
        other => {
            if let Some((_, template)) = configured.iter().find(|(k, _)| k == other) {
                return Ok(Format::Template(template.clone()));
            }
            let mut names = vec!["url".to_string(), "markdown".to_string()];
            for (k, _) in configured {
                if !names.iter().any(|n| n == k) {
                    names.push(k.clone());
                }
            }
            Err(format!("unknown format `{other}` (available: {})", names.join(", ")).into())
        }
    }
}

/// Last path segment of the URL, query string stripped.
fn url_fname(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
}
