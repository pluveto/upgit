# ![upgit](https://cdn.jsdelivr.net/gh/pluveto/upgit/logo.png)

<img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" /> <img src="https://img.shields.io/badge/Ubuntu-E95420?style=for-the-badge&logo=ubuntu&logoColor=white" /> <img src="https://img.shields.io/badge/mac%20os-000000?style=for-the-badge&logo=apple&logoColor=F0F0F0" />

**语言**: [English](../README.md) / 简体中文

*Upgit* 是一个本地命令行工具：把文件或剪贴板传到远端，打印直链。

可作为 [Typora](https://support.typora.io/Upload-Image/#image-uploaders) 的自定义图片上传器，也可配合 [VSCode 扩展](https://github.com/pluveto/upgit-vscode-extension) 使用。

无参数运行 `upgit`，或 `upgit -h`，会打印帮助。

## 支持的图床

GitHub 只是其中一种。若不想把仓库设为**公开**，请改用 SM.MS、S3 或 OSS —— GitHub 私有仓库的 raw 直链会 404。

- **GitHub** — 仓库必须公开
- **Amazon S3** 及兼容存储：MinIO、Cloudflare R2、Backblaze、Wasabi、DigitalOcean Spaces
- **码云 Gitee** — 不适合当公开图床，会拦截图床仓库
- **腾讯云 COS**、**七牛云 Kodo**、**又拍云**、**阿里云 OSS**
- **SM.MS**、**Imgur**、**ImgUrl.org**、**CatBox**、**Hello**、**牛图 Niupic**
- **LSkyPro**、**Chevereto**、**ImgBB**、**Cloudinary**、**EasyImage**
- **DALEXNI**、**img.tg**、**掘金**、**萌图 Moetu**、**网易**、**搜狗**、**upload.cc**

本机查看 id：`upgit uploaders`。完整配置表在仓库的 [`config.sample.toml`](../config.sample.toml)（**不会**打进 zip）。

## 下载

从 [Releases](https://github.com/pluveto/upgit/releases) 下载最新 zip：

| 你的系统 | 下载 |
| --- | --- |
| Windows x64 | `upgit_win_amd64.zip` |
| Windows ARM | `upgit_win_arm64.zip` |
| Linux x64 | `upgit_linux_amd64.zip` |
| Linux ARM | `upgit_linux_arm64.zip` |
| macOS Intel | `upgit_macos_amd64.zip` |
| macOS Apple Silicon | `upgit_macos_arm64.zip` |

每个 zip 里是 `upgit` 可执行文件、一份 GitHub 用的 `config.toml`，以及 `recipes/`。解压后如有需要，把二进制改名为 `upgit`（Windows 为 `upgit.exe`），把该目录加入 `PATH`。Linux / macOS 若无法执行，再 `chmod +x upgit`。

本程序不会自动检查更新。关心新版本可以点右上角 ⭐ star。

## 三步上手（GitHub）

1. 运行 `upgit init`。它会把一份 GitHub 用的 `config.toml` 写到系统配置目录，并**打印完整路径**（也可指定路径：`upgit init ./config.toml`）。
2. 打开该文件，填写 `pat`、`username`、`repo`、`branch`。`branch` 必须和仓库默认分支一致（一般是 `main`）。
3. 上传：`upgit logo.png`

```toml
default = "github"

[uploaders.github]
pat = "PASTE_YOUR_TOKEN"
username = "your-user"
repo = "your-public-repo"
branch = "main"
```

仓库**必须公开**。私有仓库的 raw 直链会 404。

**PAT** 二选一：

- 经典 token，勾选 `repo` 权限；或
- fine-grained token，对该仓库授予 **Contents: Read and write**

在 [GitHub Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens) 创建。

## 更快的几条路

已经有公开 GitHub 仓库的话，上面即可。只想马上拿到链接，下面这些更快。

### SM.MS（一个 token）

在 [sm.ms/home/apitoken](https://sm.ms/home/apitoken) 创建 token：

```toml
default = "smms"

[uploaders.smms]
token = "YOUR_SMMS_TOKEN"
```

### S3、OSS、COS、七牛（密钥 + 桶 + 域名）

Amazon S3（以及 MinIO / R2 / Backblaze / Wasabi / Spaces）、阿里云 OSS、腾讯云 COS、七牛云 Kodo 都是同一类：AccessKey、Secret、Bucket，再加你希望打印出来的公网域名。

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

把 `default` 设成表名，或临时用 `-u smms` / `-u s3` / …。更多图床见 [`config.sample.toml`](../config.sample.toml)。

## 日常用法

```bash
upgit logo.png
upgit a.png b.png
upgit logo.png -u github
upgit logo.png -t /my_images/demo          # 保留原文件名，放到该远程目录
upgit logo.png -o clipboard                # 把 URL 写入剪贴板
upgit logo.png -o clipboard -f markdown    # ![logo.png](https://...)
upgit --clipboard                          # 上传剪贴板里的截图
upgit --clipboard-files                    # 上传剪贴板里复制的文件
```

默认大小限制 **5MiB**。用 `-s` 指定字节数；`-s 0` 表示不限制。

`--raw` 输出未经 `[link]` 替换的原始 URL。`--clean` 上传成功后删除本地文件。`--verbose` 打印更多细节。

### 截图上传

1. 把截图放进剪贴板：
   - Windows：<kbd>Win</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd>
   - macOS：<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Cmd</kbd>+<kbd>4</kbd>
   - Linux：<kbd>PrintScreen</kbd>
2. 运行 `upgit --clipboard`

Linux 上读写剪贴板需要 **xclip**（X11）或 **wl-clipboard**（Wayland）。

`--clipboard-files` 上传已复制的文件：Windows 走资源管理器「复制文件」（`CF_HDROP`）；Linux / macOS 走文本路径列表。

### 把 URL 存进剪贴板

```bash
upgit logo.png --output clipboard
upgit --clipboard -o clipboard -f markdown
```

内置 `-f`：`url`（默认）和 `markdown`（`![{url_fname}]({url})`）。可在配置的 `[output_formats]` 里增加命名格式。

### 配合 Typora

> 假设 Windows 路径为 `C:\path\to\upgit.exe`

选择 **文件 → 偏好设置…**

![Typora 偏好设置](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373863.png)

转到 **图像**。将 **图像上传器** 设为 **自定义命令**，在 **命令** 里填入 `upgit` 的完整路径。点 **验证图片上传器**（Test Uploader）确认可用。

![Typora 自定义命令](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373868.png)

### AHK 一键截图上传（Windows）

1. 安装 AutoHotkey。
2. 保存并运行 `upload_clipboard.ahk`：

```ahk
; Ctrl+F9 上传剪贴板截图，并把 Markdown 写入剪贴板
^F9::
RunWait, "upgit.exe" --clipboard --output clipboard --format markdown
return
```

3. 用 <kbd>Win</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> 截图，再按 <kbd>Ctrl</kbd>+<kbd>F9</kbd> 上传并复制链接。

## 配置

### 配置文件在哪

`upgit init`（不带路径）会写到：

- Unix：`$XDG_CONFIG_HOME/upgit/config.toml`，否则 `~/.config/upgit/config.toml`
- Windows：`%APPDATA%\upgit\config.toml`

上传时的搜索顺序：

1. `--config PATH`（`-c`）
2. 当前目录的 `./config.toml`
3. Unix：`$XDG_CONFIG_HOME/upgit/config.toml` 或 `~/.config/upgit/config.toml`
4. Windows：`%APPDATA%\upgit\config.toml`，然后 `%USERPROFILE%`
5. `upgit` 可执行文件同目录下的 `config.toml`

### 重命名占位符

`naming`（别名 `rename`）是远程对象键。路径里的 `/` 会创建目录。

| 占位符 | 含义 |
| --- | --- |
| `{year}` `{month}` `{day}` | UTC 日期（来自时间戳），如 `2026` `09` `01` |
| `{hour}` `{minute}` `{second}` | UTC 时、分、秒（来自时间戳） |
| `{unix}` | Unix 时间戳（秒） |
| `{unix_tsms}` | Unix 时间戳（毫秒，高频上传更不容易撞名） |
| `{stem}` `{fname}` | 原文件名，不含扩展名（`logo`） |
| `{fullname}` | 原文件名，含扩展名（`logo.png`） |
| `{ext}` | 扩展名，带点（`.png`）；没有则为空 |
| `{hmac}` | 对 `hmac_format` 做 HMAC-SHA256（需要 `hmac_key`） |
| `{fname_hash}` | 原文件名（含扩展名，如 `logo.png`）的 MD5（十六进制） |
| `{fname_hash4}` `{fname_hash8}` | 上述 MD5 的前 4 / 8 位 |

```toml
naming = "{year}/{month}/upgit_{year}{month}{day}_{unix}{ext}"
```

使用 `{hmac}` 时还需设置 `hmac_key`，可选 `hmac_format`、`hmac_len`。

### CDN / URL 替换

上传后按 `[link]` 做子串替换：

```toml
[link]
"raw.githubusercontent.com" = "cdn.jsdelivr.net/gh"
"/main" = "@main"
```

### 输出格式

```toml
[output_formats]
"bbcode" = "[img]{url}[/img]"
"html" = '<img src="{url}" />'
"markdown-simple" = "![]({url})"
```

占位符：`{url}`、`{url_fname}`（URL 里的文件名）。

```bash
upgit --clipboard -o clipboard -f bbcode
```

### 环境变量

任意配置项都可用 `UPGIT_` 设置，嵌套用 `__`（Kong 风格）：

```bash
export UPGIT_DEFAULT=smms
export UPGIT_UPLOADERS__GITHUB__PAT=PASTE_YOUR_TOKEN
export UPGIT_UPLOADERS__GITHUB__USERNAME=your-user
export UPGIT_NAMING='{year}/{month}/{fname}_{unix}{ext}'
```

### 大小限制

默认 **5MiB**。可在配置里写 `size_limit`（字节），或用 `-s`。`-s 0` 表示不限制。

## 从源码安装

```bash
cargo install --git https://github.com/pluveto/upgit --branch next
```

或克隆本仓库后：

```bash
cargo install --path crates/upgit
```

若拉取 crates.io 较慢，可设 `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`，或使用 rsproxy 等镜像。

## 附录：从 0.2 升级

若你用过之前的 Go 版本，命令行参数、配置路径和七牛凭证有变化。见 [从 0.2 升级](upgrade-from-0.2.md)。
