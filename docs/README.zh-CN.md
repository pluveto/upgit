# upgit

把文件（或剪贴板）传到远端，打印直链。0.3 是 Rust 重写，**不兼容** 0.2：没有 JSONC 扩展、没有 `:clipboard`、七牛不要再填会过期的 upload token。

## 安装

从 [Releases](https://github.com/pluveto/upgit/releases) 下载 **zip**（`v0.3.0-alpha.2` 起）。包内是 `upgit`、`config.toml`（仅 GitHub）和 `recipes/`。解压后在 `config.toml` 里填 GitHub 字段，然后 `upgit logo.png`。

从源码安装时，`upgit init` 会写出同一份 GitHub `config.toml` 并解出 `recipes/`。完整图床目录在仓库的 [`config.sample.toml`](../config.sample.toml)，**不会**打进 zip。中文说明见本文档；中文示例仍在仓库的 `config.sample.zh-CN.toml`。

不要手拷 JSONC。默认走 GitHub。

或从源码：

```bash
cargo install --path crates/upgit
```

国内编译如拉取 crates.io 慢，可设 `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`，或使用 rsproxy。

## 默认：GitHub

建一个**公开**仓库，再申请带 `repo` 权限的 PAT。私有仓库的 raw 直链会 404。

```bash
upgit init
# 编辑 config.toml
upgit logo.png
```

```toml
default = "github"
# `default_uploader` 是 `default` 的别名。

[uploaders.github]
pat = "..."
username = "..."
repo = "..."
branch = "master"
```

不需要 `extensions` 文件夹。表名能对上内置类型或配方 id 时不用写 `type`。

## 七牛（可选，国内 CDN）

码云会拦截「当图床用」的公开仓库。GitHub raw 在国内经常抽风。七牛是内置上传器：每次上传用 **AK/SK 现场签发** token，不要用七牛网页生成的短时 token。

```toml
default = "qiniu"

[uploaders.qiniu]
access_key = "..."
secret_key = "..."
bucket = "..."
public_base = "https://cdn.example.com/"
```

`prefix` 等价于 `public_base`。

## 上传器

内置：`github`、`s3`、`aliyunoss`、`qcloudcos`、`upyun`、`qiniu`。

HTTP 配方（表名即配方 id，不必写 `type = "http"`）：`smms`、`imgur`、`catbox`、`cloudinary`、`easyimage`、`lskypro`、`lskypro2`、`hello`、`niupic`、`imgurlorg`、`imgbb`、`chevereto`、`gitee`、`dalexni`、`imgtg`、`juejin`、`moetu`、`netease`、`sougou`、`upload_cc`。

发布包里仍有 `recipes/`。`upgit init` 会写出 GitHub 用的 `config.toml` 并解出这些配方。要用其他图床，从仓库的 [`config.sample.toml`](../config.sample.toml) 拷一段表即可。

码云仍出现在示例配置里，以便对齐 0.2。不要用 Gitee 当公开图床，他们会拦截图床仓库。

## 使用

```bash
upgit logo.png
upgit --clipboard
upgit --clipboard-files
upgit logo.png -u github -o clipboard
```

Typora：图像 → 自定义命令 → `upgit` 路径。
