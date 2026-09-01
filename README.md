# ![upgit](https://cdn.jsdelivr.net/gh/pluveto/upgit/logo.png)

<img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" /> <img src="https://img.shields.io/badge/Ubuntu-E95420?style=for-the-badge&logo=ubuntu&logoColor=white" /> <img src="https://img.shields.io/badge/mac%20os-000000?style=for-the-badge&logo=apple&logoColor=F0F0F0" />

**Languages**: English / [简体中文](docs/README.zh-CN.md)

*Upgit* is a native CLI: upload a file or the clipboard, and print a public URL.

It works as a [Typora](https://support.typora.io/Upload-Image/#image-uploaders) custom image uploader, and with the [VSCode extension](https://github.com/pluveto/upgit-vscode-extension).

Run `upgit` or `upgit -h` for help.

## Supported hosts

GitHub is one option among many. If you do not want a **public** GitHub repo, use SM.MS, S3, or OSS instead — private GitHub raw URLs 404.

- **GitHub** — public repository required
- **Amazon S3** and S3-compatible storage: MinIO, Cloudflare R2, Backblaze, Wasabi, DigitalOcean Spaces
- **Gitee** — unsuitable as a public image host; they block 图床 repositories
- **Tencent Cloud COS**, **Qiniu Kodo**, **Upyun**, **Aliyun OSS**
- **SM.MS**, **Imgur**, **ImgUrl.org**, **CatBox**, **Hello**, **Niupic**
- **LSkyPro**, **Chevereto**, **ImgBB**, **Cloudinary**, **EasyImage**
- **DALEXNI**, **img.tg**, **Juejin**, **Moetu**, **NetEase**, **Sogou Pic**, **upload.cc**

List ids on your machine: `upgit uploaders`. Copy a ready-made table from [`config.sample.toml`](config.sample.toml) in this repo (not packed in the zip).

## Download

Get the latest zip from [Releases](https://github.com/pluveto/upgit/releases):

| You have | Download |
| --- | --- |
| Windows x64 | `upgit_win_amd64.zip` |
| Windows ARM | `upgit_win_arm64.zip` |
| Linux x64 | `upgit_linux_amd64.zip` |
| Linux ARM | `upgit_linux_arm64.zip` |
| macOS Intel | `upgit_macos_amd64.zip` |
| macOS Apple Silicon | `upgit_macos_arm64.zip` |

Each zip contains the `upgit` binary, a GitHub `config.toml`, and `recipes/`. Unzip, rename the binary to `upgit` (Windows: `upgit.exe`) if needed, and add that folder to `PATH`. On Linux/macOS, run `chmod +x upgit` if the file is not already executable.

There is no auto-updater. Star the repo if you want to notice new releases.

## Three steps (GitHub)

1. Run `upgit init`. It writes a GitHub `config.toml` to the platform config directory and **prints the full path** (or pass a path: `upgit init ./config.toml`).
2. Edit that file: `pat`, `username`, `repo`, `branch`. `branch` must match the repository default branch (usually `main`).
3. Upload: `upgit logo.png`

```toml
default = "github"

[uploaders.github]
pat = "PASTE_YOUR_TOKEN"
username = "your-user"
repo = "your-public-repo"
branch = "main"
```

The repository **must be public**. Private repos make raw URLs 404.

**PAT** — either:

- a classic token with the `repo` scope, or
- a fine-grained token with **Contents: Read and write** on that repository

Create a token at [GitHub Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens).

## Faster paths

GitHub is fine if you already have a public repo. These are quicker if you only want a link.

### SM.MS (one token)

Create a token at [sm.ms/home/apitoken](https://sm.ms/home/apitoken):

```toml
default = "smms"

[uploaders.smms]
token = "YOUR_SMMS_TOKEN"
```

### S3, OSS, COS, Qiniu (key + bucket + domain)

Same idea for Amazon S3 (and MinIO / R2 / Backblaze / Wasabi / Spaces), Aliyun OSS, Tencent Cloud COS, and Qiniu Kodo: access key, secret, bucket, and the public URL you want printed.

```toml
default = "s3"

[uploaders.s3]
region = "us-west-2"
bucket_name = "my-bucket"
access_key = "..."
secret_key = "..."
endpoint = "https://s3.us-west-2.amazonaws.com"
url_format = "{endpoint}/{bucket}/{path}"
```

```toml
default = "aliyunoss"

[uploaders.aliyunoss]
endpoint = "https://oss-cn-shanghai.aliyuncs.com"
access_key_id = "..."
access_key_secret = "..."
bucket_name = "your-bucket"
host = "https://cdn.example.com"
```

```toml
default = "qcloudcos"

[uploaders.qcloudcos]
host = "xxx.cos.ap-chengdu.myqcloud.com"
secret_id = "..."
secret_key = "..."
```

```toml
default = "qiniu"

[uploaders.qiniu]
access_key = "..."
secret_key = "..."
bucket = "..."
public_base = "https://cdn.example.com/"
```

Set `default` to the table name, or pass `-u smms` / `-u s3` / … for one shot. More hosts: [`config.sample.toml`](config.sample.toml).

## Everyday use

```bash
upgit logo.png
upgit a.png b.png
upgit logo.png -u github
upgit logo.png -t /my_images/demo          # keep original filename under that remote directory
upgit logo.png -o clipboard                # copy the URL
upgit logo.png -o clipboard -f markdown    # ![logo.png](https://...)
upgit --clipboard                          # screenshot on the clipboard
upgit --clipboard-files                    # files copied on the clipboard
```

Default size limit is **5MiB**. Raise it with `-s` (bytes); `-s 0` means unlimited.

`--raw` prints the URL before `[link]` replacements. `--clean` deletes the local file after a successful upload. `--verbose` prints extra detail.

### Screenshot to URL

1. Take a screenshot onto the clipboard:
   - Windows: <kbd>Win</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd>
   - macOS: <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Cmd</kbd>+<kbd>4</kbd>
   - Linux: <kbd>PrintScreen</kbd>
2. Run `upgit --clipboard`

On Linux, clipboard access needs **xclip** (X11) or **wl-clipboard** (Wayland).

`--clipboard-files` uploads copied files: on Windows, File Explorer copy (`CF_HDROP`); on Linux and macOS, a text list of paths.

### Save the URL to the clipboard

```bash
upgit logo.png --output clipboard
upgit --clipboard -o clipboard -f markdown
```

Built-in `-f` values: `url` (default) and `markdown` (`![{url_fname}]({url})`). Add more names under `[output_formats]` in config.

### Typora

> Example Windows path: `C:\path\to\upgit.exe`

**File → Preferences…**

![Typora preferences](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373863.png)

Open **Image**. Set **Image Uploader** to **Custom Command**, and put the full path to `upgit` in **Command**. Click **Test Uploader** to confirm it works.

![Typora custom command](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373868.png)

### One-shot screenshot (AHK, Windows)

1. Install AutoHotkey.
2. Save `upload_clipboard.ahk` and run it:

```ahk
; Ctrl+F9 uploads the clipboard image and copies Markdown
^F9::
RunWait, "upgit.exe" --clipboard --output clipboard --format markdown
return
```

3. <kbd>Win</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> to snip, then <kbd>Ctrl</kbd>+<kbd>F9</kbd> to upload and copy the link.

## Config

### Where the file is

`upgit init` (no path) writes:

- Unix: `$XDG_CONFIG_HOME/upgit/config.toml`, or `~/.config/upgit/config.toml`
- Windows: `%APPDATA%\upgit\config.toml`

Search order when you upload:

1. `--config PATH` (`-c`)
2. `./config.toml` in the current directory
3. Unix: `$XDG_CONFIG_HOME/upgit/config.toml` or `~/.config/upgit/config.toml`
4. Windows: `%APPDATA%\upgit\config.toml`, then `%USERPROFILE%`
5. `config.toml` next to the `upgit` binary

### Rename placeholders

`naming` (alias `rename`) is the remote object key. `/` creates directories.

| Placeholder | Meaning |
| --- | --- |
| `{year}` `{month}` `{day}` | UTC date from the timestamp, e.g. `2026` `09` `01` |
| `{hour}` `{minute}` `{second}` | UTC time from the timestamp |
| `{unix}` | Unix time in seconds |
| `{unix_tsms}` | Unix time in milliseconds (better against collisions) |
| `{stem}` `{fname}` | Original name without extension (`logo`) |
| `{fullname}` | Original name with extension (`logo.png`) |
| `{ext}` | Extension including the dot (`.png`), or empty |
| `{hmac}` | HMAC-SHA256 of `hmac_format` (needs `hmac_key`) |
| `{fname_hash}` | MD5 of the original file name with extension (`logo.png`) |
| `{fname_hash4}` `{fname_hash8}` | First 4 / 8 hex digits of that MD5 |

```toml
naming = "{year}/{month}/upgit_{year}{month}{day}_{unix}{ext}"
```

For `{hmac}` also set `hmac_key`, optionally `hmac_format` and `hmac_len`.

### CDN / URL rewrite

After upload, substring replacements in `[link]`:

```toml
[link]
"raw.githubusercontent.com" = "cdn.jsdelivr.net/gh"
"/main" = "@main"
```

### Output formats

```toml
[output_formats]
"bbcode" = "[img]{url}[/img]"
"html" = '<img src="{url}" />'
"markdown-simple" = "![]({url})"
```

Placeholders: `{url}`, `{url_fname}` (basename from the URL).

```bash
upgit --clipboard -o clipboard -f bbcode
```

### Environment variables

Any config key can be set with `UPGIT_` and `__` for nesting (Kong-style):

```bash
export UPGIT_DEFAULT=smms
export UPGIT_UPLOADERS__GITHUB__PAT=PASTE_YOUR_TOKEN
export UPGIT_UPLOADERS__GITHUB__USERNAME=your-user
export UPGIT_NAMING='{year}/{month}/{fname}_{unix}{ext}'
```

### Size limit

Default **5MiB**. Override with `size_limit` in config (bytes) or `-s`. `-s 0` disables the limit.

## Build from source

```bash
cargo install --git https://github.com/pluveto/upgit --branch next
```

Or clone this repo and:

```bash
cargo install --path crates/upgit
```

If crates.io is slow, try `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse` or a mirror such as rsproxy.

## Appendix: upgrading from 0.2

If you used the previous Go release, flags, config paths, and Qiniu credentials changed. See [Upgrading from 0.2](docs/upgrade-from-0.2.md).
