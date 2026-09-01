/// Fill a 0.2-style output template.
///
/// `{url}` is the public URL, `{fname}` is the original local base name
/// (with extension), and `{url_fname}` / `{urlfname}` is the URL's last path
/// segment.
pub fn render_output(format: &str, url: &str, fname: &str) -> String {
    let url_fname = url_fname(url);
    format
        .replace("{url_fname}", url_fname)
        .replace("{urlfname}", url_fname)
        .replace("{fname}", fname)
        .replace("{url}", url)
}

/// Last path segment of the URL, query string stripped.
pub(crate) fn url_fname(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
}
