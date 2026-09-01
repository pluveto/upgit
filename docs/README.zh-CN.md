# ![upgit](https://cdn.jsdelivr.net/gh/pluveto/upgit/logo.png)

<img align="right" src="https://img.shields.io/github/actions/workflow/status/pluveto/upgit/ci.yml?style=flat-square" />

<img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logo=windows&logoColor=white" /> <img src="https://img.shields.io/badge/Ubuntu-E95420?style=for-the-badge&logo=ubuntu&logoColor=white" /> <img src="https://img.shields.io/badge/mac%20os-000000?style=for-the-badge&logo=apple&logoColor=F0F0F0" />

**语言**: [English](../README.md) / 简体中文

**开发**: [开发手册](DEVELOPMENT.zh-CN.md)

*Upgit* 可以快捷地将文件上传到 Github 仓库并得到其直链。简洁跨平台，不常驻内存。

可作为 [Typora](https://support.typora.io/Upload-Image/#image-uploaders) 的自定义上传器使用，也可配合 [VSCode 扩展](https://github.com/pluveto/upgit-vscode-extension)。

**太长不看**：本程序用于快速上传。配合 AHK 可以帮助你一键完成截图、上传、复制链接的操作。

## 特点

+ 通过 [VSCode 扩展](https://github.com/pluveto/upgit-vscode-extension) 集成 VSCode
+ 支持多平台，包括 Linux、Windows 和 macOS
+ 支持**多种上传器**，目前包括 Github 和 SMMS
+ 不限制文件类型
+ 支持从**剪贴板上传**
+ 自定义**自动重命名**规则（包括路径）
+ 可通过替换规则实现**CDN**加速
+ 可通过**环境变量**配置
+ 将 URL 输出到标准输出/**剪贴板**，支持 Markdown 格式

### 上传扩展

+ Github
+ S3 兼容存储
   + AWS S3
   + MinIO
   + Cloudflare R2
   + Ceph
   + Backblaze
   + Flexify.IO
   + IBM Cloud Object Storage
   + DigitalOcean Spaces
   + Wasabi
+ Gitee（会拦截公开图床仓库，建议改用 SM.MS / S3 / OSS）
+ 腾讯云 COS
+ 七牛云 Kodo
+ 又拍云
+ 阿里云 OSS
+ Hello
+ Niupic
+ SM.MS
+ Imgur
+ ImgUrl.org
+ CatBox
+ LSkyPro
+ Chevereto
+ ImgBB
+ Cloudinary
+ EasyImage
+ DALEXNI
+ img.tg
+ 掘金
+ 萌图 Moetu
+ 网易
+ 搜狗
+ upload.cc
+ Lsky Pro v2

查看更多: `upgit uploaders`

## 开始使用

### 下载

从 [Releases](https://github.com/pluveto/upgit/releases) 下载最新 zip。

> 如果不知道下载哪一个：
>
> + 对于大多数 Windows 用户，请选择 `upgit_win_amd64.zip`
> + 对于 Windows ARM，请选择 `upgit_win_arm64.zip`
> + 对于 Windows 32 位，请选择 `upgit_win_386.zip`
> + 对于 Linux x64，请选择 `upgit_linux_amd64.zip`
> + 对于 Linux arm64，请选择 `upgit_linux_arm64.zip`
> + 对于 Linux 32 位，请选择 `upgit_linux_386.zip`
> + 对于 Linux ARM，请选择 `upgit_linux_arm.zip`
> + 对于大多数 macOS 用户，请选择 `upgit_macos_arm64.zip`
> + 对于 macOS Intel，请选择 `upgit_macos_amd64.zip`
> + 如需执行权限，请运行 `chmod +x upgit`

解压后将其重命名为 `upgit`（对于 Windows 用户，`upgit.exe`），保存到某处。若要从任何地方访问它，请将其目录添加到 `PATH` 环境变量中。

运行 `upgit update` 可把本程序更新到最新正式版。`upgit update --beta` / `--alpha` 分别允许 beta / alpha 通道。不会覆盖 `config.toml` 和你改过的 recipes。

### 配置

运行 `upgit init`。它会写入一份 GitHub 用的 `config.toml` 并打印路径。也可以在程序的同一目录创建 `config.toml`。

内容按照 [此示例配置文件](../config.sample.zh-CN.toml) 填写即可。

仓库**必须公开**。私有仓库的 raw 直链会 404。

个人访问令牌（PAT）：经典 token 勾选 `repo` 权限，或 fine-grained token 对该仓库授予 **Contents: Read and write**。在 [GitHub Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens) 创建。

### 使用

比如上传 `logo.png` 并自动使用重命名规则，执行：

```shell
./upgit logo.png
# for windows: .\upgit.exe logo.png
```

然后会看到一个指向  `logo.png` 的直链。

比如上传 `logo.png`  到远程文件夹 `/my_images/demo`，执行：

```shell
./upgit logo.png -t /my_images/demo
# 对于 Windows: .\upgit.exe logo.png -t /my_images/demo
```

有关更多帮助，请键入 `-h` 参数

```shell
Upload anything to github repo or other remote storages and then get its link.

Usage: upgit [OPTIONS] [FILE]...
       upgit <COMMAND>

Commands:
  init       Write a GitHub config.toml
  update     Replace this binary with a GitHub release (does not overwrite config.toml)
  uploaders  List built-in uploaders
  help       Print this message or the help of the given subcommand(s)

Arguments:
  [FILE]...  Local files to upload

Options:
      --clipboard                Upload the image currently on the clipboard
      --clipboard-files          Upload files copied on the clipboard (file list)
  -u, --uploader <UPLOADER>      Uploader id (see `upgit uploaders`)
  -o, --output <OUTPUT>          stdout or clipboard (clipboard copies the URL) [default: stdout] [alias: --output-type] [possible values: stdout, clipboard]
  -f, --format <FORMAT>          url | markdown | named [alias: --output-format]
  -t, --target-dir <TARGET_DIR>  Keep the original filename under this remote directory
  -s, --size-limit <SIZE_LIMIT>  Maximum file size in bytes (0 = unlimited)
  -c, --config <CONFIG>          Path to a TOML config file [alias: --config-file]
  -r, --raw                      Skip [link] replacements
  -C, --clean                    Delete local files after a successful upload
  -V, --verbose                  Print uploader, object key, and URL to stderr
  -w, --wait                     Do not exit after upload until the user presses a key
  -n, --no-log                   Disable writing upgit.log and history.log
  -j, --jobs <N>                 Concurrent uploads (1 = serial) [default: 1]
      --application-path <PATH>  Directory that owns config.toml / upgit.toml, history.log, and upgit.log
      --version                  Print version
  -h, --help                     Print help

Uploaders (pass --uploader ID, or set default in config.toml):
  github      GitHub
  s3          Amazon S3 (MinIO / Cloudflare R2 / Backblaze / Wasabi / DigitalOcean Spaces / Ceph / Flexify.IO / IBM Cloud Object Storage)
  aliyunoss   Aliyun OSS
  qcloudcos   Tencent Cloud COS
  upyun       Upyun
  qiniu       Qiniu Kodo
  smms        SM.MS
  imgur       Imgur
  catbox      Catbox
  cloudinary  Cloudinary
  easyimage   EasyImage
  lskypro     Lsky Pro
  lskypro2    Lsky Pro v2
  hello       Helloimg
  niupic      Niupic
  imgurlorg   ImgURL.org
  imgbb       ImgBB
  chevereto   Chevereto
  gitee       Gitee
  dalexni     DALEXNI
  imgtg       img.tg
  juejin      Juejin
  moetu       Moetu
  netease     NetEase
  sougou      Sogou
  upload_cc   upload.cc

Create a config with `upgit init`. Update with `upgit update`. List ids with `upgit uploaders`.
https://github.com/pluveto/upgit
```

### 配合 Typora 使用

> 假设 *upgit* 程序保存在 `"C:\repo\upgit\upgit.exe"`。

选择 *文件 > 首选项*

![image-20220128204217802](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373863.png)

转到 *Image*。选择 *自定义命令* 作为 *图像上传器*。

在 *命令* 文本框中输入 *upgit* 程序位置。

> 你可以点击 *测试上传*（Test Uploader）按钮来确保它工作正常。

![image-20220128204418723](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373868.png)

然后就可以使用了。

### 上传剪贴板图像

上传剪贴板图像（仅支持 **png** 格式）：

```shell
./upgit --clipboard
```

截图快捷键：

+ 在 macOS 上，使用 `Ctrl+Shift+Cmd+4`
+ 在 Linux/Ubuntu 上，使用 `Ctrl+Shift+PrintScreen`
+ 在 Windows 上，使用 `Shift+Win+s`

Linux 上读写剪贴板需要 **xclip**（X11）或 **wl-clipboard**（Wayland）。

**兼容 Snipaste**（Windows）：Snipaste 的位图截图可以同样上传。

### 上传剪贴板文件

**注意：**此功能仅在 Windows 上支持。

上传资源管理器中复制的文件（`CF_HDROP`）：

```shell
./upgit --clipboard-files
```

### 将 URL 保存到剪贴板

使用参数 `--output clipboard`（别名 `--output-type`）：

```shell
upgit logo.png --output clipboard
# or .\upgit.exe --clipboard -o clipboard
```

#### 复制为 Markdown 格式

增加参数 `-f markdown`（别名 `--output-format`）：

```shell
upgit logo.png --output clipboard -f markdown
# or .\upgit.exe --clipboard -o clipboard -f markdown
```

然后会在剪贴板上得到一个 Markdown 图片链接，比如：

```md
![logo.png](https://cdn.jsdelivr.net/gh/pluveto/upgit/logo.png)
```

### AHK 的最佳实践

对于 Windows 用户：

1. 安装 AHK
2. 创建这个脚本 `upload_clipboard.ahk` 并运行：

   ```ahk
   ; Press Ctrl + F9 to upload clipboard image
   ^F9::
   RunWait, "upgit.exe" --clipboard --output clipboard --format markdown
   return
   ```

3. 然后按 <kbd>Win</kbd><kbd>Shift</kbd><kbd>S</kbd> 截图，按 <kbd>Ctrl</kbd><kbd>F9</kbd> 上传并将其链接复制到剪贴板

## 配置文件说明

| 键                   | 说明                                                         |
| --------------------- | ------------------------------------------------------------ |
| username              | 您的 Github 用户名，例如 `pluveto` |
| repo                  | 您的 Github 存储库名称，例如 `upgit` |
| branch                | 保存文件的分支，例如 `master` 或 `main` |
| pat                   | 个人访问令牌。 访问 [GitHub 文档](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) 了解更多信息 |
| rename                | 重命名规则。不存在的路径目录将被创建。 支持下列占位符： |
| -- `{year}`           | -- 年份，如 `2006`                                       |
| -- `{month}`          | -- 月，如 `01`                                       |
| -- `{day}`            | -- 日，如 `02`                                         |
| -- `{hour}`            | -- 时        |
| -- `{minute}`            | -- 分  |
| -- `{second}`            | -- 秒  |
| -- `{unix_ts}`        | -- 以秒计的 Unix 时间戳，如 `1643373370`. |
| -- `{unix_tsms}`        | -- 以毫秒计的 Unix 时间戳，如 `1644212979622`. |
| --- `{ext}`           | -- 扩展名，如 `.png`，若文件无扩展名，则为空串 |
| -- `{fname}`      | -- 原始文件名，如 `logo` （不含扩展名） |
| -- `{fname_hash}` | -- `{fname}`（不含扩展名）的 MD5 散列值               |
| -- `{fname_hash4}` | -- `{fname}` 的 MD5 散列值，取前 4 位               |
| -- `{fname_hash8}` | -- `{fname}` 的 MD5 散列值，取前 8 位               |
| -- `{hmac}`        | -- 对 `hmac_format` 做 HMAC-SHA256，截断到 `hmac_len`            |
| hmac_key           | 计算 `{hmac}` 所用的密钥                                          |
| hmac_format        | `{hmac}` 的格式字符串。支持上面全部占位符。   |
| hmac_len           | `{hmac}` 散列长度。0 表示不截断。                              |

这是一个简单的示例配置文件：

```toml
rename = "{year}/{month}/upgit_{year}{month}{day}_{unix_ts}{ext}"
[uploaders.github]
pat = "ghp_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
repo = "repo-name"
username = "username"
branch = "main"
```

配置文件按以下顺序查找：

1. `--config` / `--config-file`
2. `./config.toml`
3. `$XDG_CONFIG_HOME/upgit/config.toml` 或 `%APPDATA%\upgit\config.toml`
4. `~/.upgit.config.toml`
5. `~/.config/upgitrc`
6. 可执行文件同目录下的 `config.toml` 和 `upgit.toml`

`--application-path` 会改变可执行文件同目录的查找位置。每次上传后会在该目录写入 `history.log` 和 `upgit.log`。`--no-log` 时不写 `history.log` 和 `upgit.log`。默认一次只上传一个文件；只有指定 `--jobs` 才会并发。

### 通过环境变量配置

+ `UPGIT_TOKEN`
+ `UPGIT_RENAME`
+ `UPGIT_USERNAME`
+ `UPGIT_REPO`
+ `UPGIT_BRANCH`

任意配置项也可用 `UPGIT_` 设置，嵌套用 `__`（Kong 风格）：

```bash
export UPGIT_DEFAULT=smms
export UPGIT_UPLOADERS__GITHUB__PAT=PASTE_YOUR_TOKEN
export UPGIT_UPLOADERS__GITHUB__USERNAME=your-user
export UPGIT_NAMING='{year}/{month}/{fname}_{unix}{ext}'
```

### 自定义输出格式

可以通过如下方式自定义输出格式：

```toml
[output_formats]
"bbcode" = "[img]{url}[/img]"
"html" = '<img src="{url}" />'
"markdown-simple" = "![]({url})"
```

占位符：

+ `{url}`：图片 URL
+ `{fname}`：原始文件名
+ `{url_fname}`：URL 里的文件名

使用方法示例：

```
upgit --clipboard -o clipboard -f bbcode
```

## 附录：从 0.2 升级

若你用过之前的 Go 0.2 版本，`:clipboard`、`--output-type` 等参数、配置路径和七牛凭证有变化。没有 `extensions/` 目录。见 [从 0.2 升级](upgrade-from-0.2.md)。

可选：`cargo install --git https://github.com/pluveto/upgit`
