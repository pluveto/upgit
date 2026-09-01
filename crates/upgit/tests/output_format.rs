use upgit::render_output;

#[test]
fn render_output_replaces_url_and_fname() {
    let url = "https://cdn.example.com/2022/01/renamed.png";
    let fname = "logo.png";
    assert_eq!(
        render_output("[img]{url}[/img]", url, fname),
        "[img]https://cdn.example.com/2022/01/renamed.png[/img]"
    );
    assert_eq!(render_output("{fname}", url, fname), "logo.png");
    assert_eq!(
        render_output("[img]{url}[/img] {fname}", url, fname),
        "[img]https://cdn.example.com/2022/01/renamed.png[/img] logo.png"
    );
}

#[test]
fn render_output_url_fname_aliases_and_markdown() {
    let url = "https://cdn.example.com/path/renamed.png?x=1";
    let fname = "logo.png";
    assert_eq!(render_output("{url_fname}", url, fname), "renamed.png");
    assert_eq!(render_output("{urlfname}", url, fname), "renamed.png");
    assert_eq!(
        render_output("![{url_fname}]({url})", url, fname),
        "![renamed.png](https://cdn.example.com/path/renamed.png?x=1)"
    );
}
