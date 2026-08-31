# upgit

Native CLI: upload a file (or the clipboard) and print a public URL.

This is the 0.3 Rust rewrite. It is **not** a drop-in for 0.2 (Go): no JSONC extensions, no `:clipboard` placeholder, no static Qiniu upload token.

**Languages**: English / [简体中文](docs/README.zh-CN.md)

## Install

Download a **zip** from [Releases](https://github.com/pluveto/upgit/releases) (`v0.3.0-alpha.2` or newer). It contains the binary, `config.sample.toml`, and `recipes/`. Unzip, then:

```bash
upgit init    # writes config.toml + recipes/ next to you
```

Do not copy JSONC by hand. Fill secrets in `config.toml` and run `upgit logo.png`.

Or from source:

```bash
cargo install --path crates/upgit
```

## Switch to Qiniu (the usual path)

Gitee will not host a public image-bed repo. GitHub raw is often slow in CN. Qiniu is first-class: **AK/SK mint a fresh upload token on every put**. Do not paste a token from the Qiniu web debugger; it expires.

```bash
upgit init
# edit config.toml
upgit logo.png
```

`config.toml` (also `config.sample.toml`):

```toml
default = "qiniu"

[uploaders.qiniu]
access_key = "..."
secret_key = "..."
bucket = "..."
public_base = "https://cdn.example.com/"
```

No `extensions/` directory. `type = "qiniu"` is optional when the table is named `qiniu`. `prefix` is accepted as an alias of `public_base`. `default_uploader` is accepted as an alias of `default`.

## Use

```bash
upgit logo.png
upgit --clipboard                    # screenshot on the clipboard
upgit --clipboard-files              # file list on the clipboard
upgit logo.png -u qiniu -o clipboard # URL onto the clipboard
upgit --help
```

Typora: Image → Custom Command → path to `upgit`.

## HTTP image hosts

SMMS / Imgur are built-in recipes:

```toml
default = "smms"

[uploaders.smms]
type = "http"
recipe = "smms"
token = "..."
```

Do not use Gitee as a public image host: they block repos used as 图床.

## Config search

`--config PATH`, else `./config.toml`, `~/.config/upgit/config.toml`, then the binary directory.
