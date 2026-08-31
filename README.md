# upgit

Native CLI: upload a file (or the clipboard) and print a public URL.

This is the 0.3 Rust rewrite. It is **not** a drop-in for 0.2 (Go): no JSONC extensions, no `:clipboard` placeholder, no static Qiniu upload token.

**Languages**: English / [简体中文](docs/README.zh-CN.md)

## Install

Download a **zip** from [Releases](https://github.com/pluveto/upgit/releases) (`v0.3.0-alpha.2` or newer). It contains the binary, `config.sample.toml`, and `recipes/`. Unzip, then:

```bash
upgit init    # writes config.toml + recipes/ next to you
```

Do not copy JSONC by hand. Fill secrets in `config.toml` and run `upgit logo.png`. GitHub is the usual path.

Or from source:

```bash
cargo install --path crates/upgit
```

## Default: GitHub

Create a **public** repository and a PAT with `repo` scope. Private repos make raw URLs 404.

```bash
upgit init
# edit config.toml
upgit logo.png
```

`config.toml` (also `config.sample.toml`):

```toml
default = "github"
# `default_uploader` is accepted as an alias of `default`.

[uploaders.github]
pat = "..."
username = "..."
repo = "..."
branch = "master"
```

No `extensions/` directory. Table names that match a first-class kind or a recipe id do not need `type`.

## Qiniu (optional, CN CDN)

Gitee will not host a public image-bed repo. GitHub raw is often slow in CN. Qiniu is first-class: **AK/SK mint a fresh upload token on every put**. Do not paste a token from the Qiniu web debugger; it expires.

```toml
default = "qiniu"

[uploaders.qiniu]
access_key = "..."
secret_key = "..."
bucket = "..."
public_base = "https://cdn.example.com/"
```

`prefix` is accepted as an alias of `public_base`.

## Uploaders

First-class: `github`, `s3`, `aliyunoss`, `qcloudcos`, `upyun`, `qiniu`.

HTTP recipes (table name = recipe id; no `type = "http"` needed): `smms`, `imgur`, `catbox`, `cloudinary`, `easyimage`, `lskypro`, `lskypro2`, `hello`, `niupic`, `imgurlorg`, `imgbb`, `chevereto`, `gitee`, `dalexni`, `imgtg`, `juejin`, `moetu`, `netease`, `sougou`, `upload_cc`.

The release zip still contains `recipes/`. `upgit init` writes the sample config and extracts those recipes.

Gitee is in the sample for 0.2 parity. Do not use it as a public image host: they block repos used as 图床.

## Use

```bash
upgit logo.png
upgit --clipboard                    # screenshot on the clipboard
upgit --clipboard-files              # file list on the clipboard
upgit logo.png -u github -o clipboard # URL onto the clipboard
upgit --help
```

Typora: Image → Custom Command → path to `upgit`.

## HTTP image hosts

SMMS / Imgur are bundled recipes:

```toml
default = "smms"

[uploaders.smms]
token = "..."
```

## Config search

`--config PATH`, else `./config.toml`, `~/.config/upgit/config.toml`, then the binary directory.
