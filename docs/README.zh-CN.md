# upgit

把文件（或剪贴板）传到远端，打印直链。0.3 是 Rust 重写，**不兼容** 0.2：没有 JSONC 扩展、没有 `:clipboard`、七牛不要再填会过期的 upload token。

## 安装

从 [Releases](https://github.com/pluveto/upgit/releases) 下载 **zip**（`v0.3.0-alpha.2` 起）。包内是二进制、`config.sample.toml` 和 `recipes/`。解压后：

```bash
upgit init    # 写出 config.toml 和 recipes/
```

不要手拷 JSONC。在 `config.toml` 里填密钥，然后 `upgit logo.png`。

或从源码：

```bash
cargo install --path crates/upgit
```

国内编译如拉取 crates.io 慢，可设 `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`，或使用 rsproxy。

## 换七牛

码云会拦截「当图床用」的公开仓库。GitHub raw 在国内经常抽风。七牛是内置上传器：每次上传用 **AK/SK 现场签发** token，不要用七牛网页生成的短时 token。

```bash
upgit init
# 编辑 config.toml
upgit logo.png
```

```toml
default = "qiniu"

[uploaders.qiniu]
access_key = "..."
secret_key = "..."
bucket = "..."
public_base = "https://cdn.example.com/"
```

不需要 `extensions` 文件夹。表名是 `qiniu` 时不用写 `type`。`prefix` 等价于 `public_base`，`default_uploader` 等价于 `default`。

## 使用

```bash
upgit logo.png
upgit --clipboard
upgit --clipboard-files
upgit logo.png -u qiniu -o clipboard
```

Typora：图像 → 自定义命令 → `upgit` 路径。

不要用 Gitee 当公开图床。
