# upgit 开发手册

给第一次进仓库的人和被拉来改代码的 AI。读完应能：把环境搭到和 CI 同一套门槛、在 `next` 上提交、用 `cargo-release` 发版。

用户怎么用产品：仓库根目录 [README.md](../README.md) / [docs/README.zh-CN.md](README.zh-CN.md)。本手册不讲产品教程。

## 1. 仓库与分支

- 默认分支是 **`next`**。日常开发、PR、发版都在这里。
- 版本号只写在根目录 `Cargo.toml` 的 `[workspace.package] version`。成员 crate 用 `version.workspace = true`，crate 之间用 `path` 依赖，不要再写一遍版本。
- 不往 crates.io 发（`publish = false`）。产物是 GitHub Release 上的 zip。

Workspace 三个成员：

| Crate | 职责 |
| --- | --- |
| `crates/upgit-core` | 领域对象：`Artifact`、`ObjectKey`、`KeyPolicy`、`LinkPolicy`、`Uploader`、`Publisher`、`Registry`、`UploadError` |
| `crates/upgit-uploaders` | 各图床对象、HTTP 配方、`AppConfig`（解析 / 环境变量 / `install_into`）、`HostCatalog` |
| `crates/upgit` | CLI：`clap` 子命令、`App`、`Intake`/`Source`、`Emitter`、`init`、history |

其它位置：`recipes/*.toml`（编进二进制的 HTTP 配方）、`config.github.toml`（init / 发布 zip 用的 GitHub 模板）、`config.sample.toml`（完整目录，不打进 zip）、`.githooks/`、`.github/workflows/`、`release.toml`、`rust-toolchain.toml`。

## 2. 三巨头约束

本仓库的设计与评审以 **Eric Evans、John Ousterhout、Alan Kay** 三人的观点为硬约束，不是装饰。改架构、加模块、写错误信息、写测试之前，先对照下面三条。做设计讨论或合并前审核时，三人必须全出场、互相质疑至少一轮，结论落到具体路径 / API / 不变量上；证据不足就写「无法核实」。

出处：Evans《领域驱动设计》；Ousterhout《软件设计哲学》；Kay，《The Early History of Smalltalk》与关于 OOP 的公开论述。

### Eric Evans：领域与边界

- **统一语言**：用户、文档、代码用同一套词。用户说「上传器」「配置」「直链」，代码里就是 `Uploader`、`AppConfig`、`Locator`/`PublicUrl`。不要在用户可见表面写内部词：`registry`、`recipe id`、`first-class`、`JSONC`、`extensions/`。
- **限界上下文**：`upgit-core` 是领域层，不知道 HTTP 客户端、不知道 clap。`upgit-uploaders` 把配置翻译成上传器对象。`upgit` 是应用层：收集本地文件、调 Publisher、把 URL 打出去。不要为了省事把 ureq / clap 拖进 core。
- **模型驱动**：远程对象键是 `KeyPolicy` 的事，不是某个 Uploader 里私自改名；CDN 替换是 `LinkPolicy`；上传是 `Uploader::upload(artifact, key)`。新行为优先落到已有对象上，而不是再开一条平行流程。

落到本仓库：

- `Registry` 只按 id 查找已经 `install_into` 的对象。CLI 报「未知上传器」时，可用列表来自 `HostCatalog`，不要在 `App` 里硬编码 GitHub/七牛示例块。
- 用户可见错误用 `UploadError { what, hint, status }`，禁止把远端 JSON/XML 原文倒给用户。

### John Ousterhout：复杂度与完成标准

- **复杂度 = 让以后更难改**。接口短、实现厚（深模块）。`Publisher::publish` 藏命名 + 上传 + 替换；调用方不要自己拼这三步。
- **错误消灭在定义阶段**。HTTP 配方缺 `token` 在 `install_into` 时失败，不要发出去再报 `invalid json response`。`naming` 含 `{hmac}` 但没有 `hmac_key`，在 `namer()` 时失败。占位 PAT（`...`、`PASTE_`、`YOUR_`）当缺字段。
- **信息隐藏**：`init` 必须是 clap 子命令，禁止在 `main` 里拦截 `args[1] == "init"`（否则 `init --help` 会写成文件）。
- **怎样算做完**：文档写了但代码没到、删测试、把失败标成「已知债」都不算完成。测试必须打到真正发出去的函数，禁止在测试里再实现一份算法然后断言两边相等。

落到本仓库：

- CI 与 git hook 同一套门槛：`cargo fmt --all -- --check`，`RUSTFLAGS=-D warnings cargo clippy --workspace --all-targets --locked`，`cargo test --workspace --locked`。
- README 不要写死版本号。版本只出现在 git tag 和 `upgit --version`。

### Alan Kay：对象、状态、消息

- **对象是自治单元**，不是「全局配置结构体 + 一堆函数」。`GithubUploader`、`QiniuUploader`、`HttpRecipeUploader` 各自持有自己的配置，只收 `upload` 消息。
- **消息传递**：`Intake` 产出 `Artifact`；`Publisher` 问 `KeyPolicy` 要键，把 `(artifact, key)` 发给 `Uploader`，再问 `LinkPolicy` 要展示 URL。调用方不打开对方的肚子改字段。
- **延迟绑定**：表名对上配方 id 或内置 kind 就不必写 `type`。换图床是换 Registry 里的对象，不是改 `match` 上传路径。
- **简单的事保持简单**：加一种 HTTP 图床，优先新配方对象（`recipes/*.toml` + catalog），不要为每家网站复制一套客户端。

反面（禁止）：

- 把上传写成 `match id { "github" => http_put_github(), ... }` 的过程式总线。
- 用大量纯函数管道代替对象上的方法，把状态拆到调用方去拼。
- 为了「函数式一点」把 `Uploader` 收成闭包或自由函数。

### 评审时怎么用

改核心抽象或用户可见行为时，用讨论（deliberate）把选项收敛；声称「做完了 / 可以发」时用审核（audit）。总判取三人中更严的一侧。主 Agent 主持判断，实现交给单独的实现回合。

## 3. 环境搭建（重点）

目标：你机器上的检查命令和 GitHub Actions `CI` job 一致。差一条，就会出现「本地绿、CI 红」。

### 3.1 克隆

```bash
git clone https://github.com/pluveto/upgit.git
cd upgit
```

默认就是 `next`。不要从别的长期分支开日常工作。

### 3.2 Rust 工具链

仓库根目录有 `rust-toolchain.toml`：

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

安装 [rustup](https://rustup.rs/)，进目录后 rustup 会按该文件拉 **stable**，并装上 `rustfmt` 和 `clippy`。确认：

```bash
rustc --version
cargo --version
rustup component list --installed   # 应有 rustfmt、clippy
```

国内拉 crates.io 慢，可：

```bash
export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
# 或配镜像，例如 rsproxy，按镜像站文档设置
```

需要系统 C 工具链才能编部分依赖（尤其 Windows 上的 `ring` 等）：

- Linux：发行版的 `build-essential` / `gcc` 即可。
- macOS：Xcode Command Line Tools（`xcode-select --install`）。
- Windows：MSVC 工具链（Visual Studio Build Tools，勾选「使用 C++ 的桌面开发」），不要混用缺失的 GNU 环境去编 `*-pc-windows-msvc`。

Linux 上剪贴板集成测 `--clipboard` 时，X11 需要 `xclip`，Wayland 需要 `wl-clipboard`。没有它们，程序会报错并提示安装，不是编译失败。

### 3.3 第一次编过

```bash
cargo build -p upgit --locked
cargo run -p upgit --locked -- --version
cargo run -p upgit --locked -- --help
cargo run -p upgit --locked -- uploaders
```

`--locked` 必须带：与 CI 一样，禁止顺手改 `Cargo.lock`。只有你有意升级依赖时才去掉 `--locked` 并提交新的 lock。

### 3.4 Git hook（与 CI 同门槛）

钩子在仓库的 `.githooks/`，**不会**在 clone 后自动生效。每个克隆执行一次：

```bash
./scripts/install-git-hooks.sh
```

它会 `git config core.hooksPath .githooks`。之后：

- **pre-commit**：暂存区里有 `.rs` / `.toml` / `Cargo.lock` 时，跑 `cargo fmt --all -- --check` 和 `cargo clippy --workspace --all-targets --locked`（`RUSTFLAGS=-D warnings`）。
- **pre-push**：无条件跑同样的 fmt + clippy。

Clippy 的警告在本仓库等于错误。`cargo fmt` 只检查、不代你改；先本地 `cargo fmt --all` 再提交。

绕过钩子（`--no-verify`）会把红的 CI 留给所有人，不要用。

本机是 Linux/macOS 时，钩子**看不到** `#[cfg(windows)]` 下的死代码。Windows 专有代码必须等 CI 的 `windows-latest`，或自己交叉/在 Windows 上再 clippy 一次。曾经因此挂过：仅非 Windows 使用的函数没加 `cfg(not(windows))`。

### 3.5 日常检查命令（与 CI 逐步对齐）

```bash
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
cargo run -p upgit --locked -- --help
cargo run -p upgit --locked -- uploaders
```

`CI` workflow（`.github/workflows/ci.yml`）在 `next` 的 push / PR 上于 ubuntu、windows、macos 各跑一遍以上步骤。

### 3.6 Git 提交签名

本仓库开发机若开了 `commit.gpgsign=true`，在没有 pinentry 的环境（常见于自动化/远程会话）会 `gpg: signing failed: Inappropriate ioctl for device`。`cargo-release` 内部调 `git commit`，一样会炸。

这个克隆可以：

```bash
git config --local commit.gpgsign false
```

`release.toml` 里 `sign-commit = false`、`sign-tag = false`，但 **挡不住** 全局 `commit.gpgsign=true`。发版前确认 `git config --show-origin commit.gpgsign`。

### 3.7 发版额外工具

发版用 [cargo-release](https://github.com/crate-ci/cargo-release)，不是手改 toml。

```bash
cargo install cargo-release --locked
cargo release -V
```

推送 tag / 改 GitHub 默认分支需要能写这个仓库的凭据。用 GitHub CLI 时确认当前用户是有权限的那一个（`gh auth status`）。

## 4. 提交说明

使用 [Conventional Commits](https://www.conventionalcommits.org/)：

- `feat:` 用户能感知的能力
- `fix:` 缺陷
- `ci:` 工作流
- `chore:` 版本号、琐碎维护（`cargo-release` 自动提交是 `chore: bump version to {{version}}`）
- `docs:` 文档
- `refactor:` 不改变用户可见行为的结构变化

主题一行，必要时正文空一行写原因。不要把实现过程写进主题。

## 5. 发版

配置在 `release.toml`。只允许在 **`next`** 上发。默认是 dry-run，必须加 `--execute` 才会改文件、提交、打 tag、push。

```bash
# 预览（不改仓库）
cargo release beta          # 0.3.0 -> 0.3.1-beta.1，或 0.3.0-beta.3 -> 0.3.0-beta.4
cargo release release       # 去掉预发布：0.3.0-beta.3 -> 0.3.0

# 真发
cargo release beta --execute --no-confirm
cargo release release --execute --no-confirm
```

注意：只写 `cargo release --execute` **不够**。不带 level 时 cargo-release 可能撞上已有 tag 直接失败。正式版请显式 `cargo release release --execute`。

它会：

1. 只改 workspace 的 `version`（以及 lockfile 里对应的三行）
2. 提交 `chore: bump version to …`
3. 打 **annotated** tag `v{{version}}`（例如 `v0.3.0`、`v0.3.0-beta.4`）
4. `git push` 分支和 tag

**不要**手改三份 crate 的 version，**不要**自己 `git tag` 应付。

推送 `v*` tag 之后，`.github/workflows/release.yml`：

1. ubuntu 上 `cargo test --workspace --locked`
2. 矩阵交叉编译 zip（linux 386/amd64/arm/arm64，windows 386/amd64/arm64，macOS amd64/arm64）
3. 每个 zip：`upgit`（Unix 可执行位 `0755`）+ `config.toml`（来自 `config.github.toml`）+ `recipes/`
4. **不**打 `config.sample.toml`
5. 创建 GitHub Release。tag 名里带 `-` 的标成 prerelease（`v0.3.0-beta.1`），`v0.3.0` 是正式版

`workflow_dispatch` 在非 tag 上跑 **不会** 发 Release（`if: github.ref_type == 'tag'`）。

发完核对：

- https://github.com/pluveto/upgit/releases 上 tag、prerelease 开关、九个 zip
- `upgit --version` 与 tag 去掉 `v` 一致

## 6. 用户可见表面（发版前扫一眼）

这些词不要出现在 README、`--help`、`init` 的 stdout、错误信息、zip 内 `config.toml` 注释、Release body（升级专页除外）：

`JSONC`、`extensions/`、`registry`、`first-class`、`recipe id`、`20 bundled`

无参数的 `upgit` 必须打印帮助并以退出码 2 离开，不能先去加载配置再报「没配上传器」。`upgit init --help` 必须是帮助，不能写出名叫 `--help` 的文件。

## 7. 给 AI 的收工检查

- 领域逻辑在 `upgit-core`，没有把 clap/ureq 引进 core。
- 新上传能力是对象上的 `upload` 消息，不是按 id 的过程式 `match`。
- 用户错误有 `what`/`hint`，没有远端 JSON/XML。
- `cargo fmt --check`、`RUSTFLAGS=-D warnings clippy`、`cargo test --locked` 全过。
- 若动了 Windows 专用代码，不能只靠 Linux 钩子签字。
- 没在 README 里写死版本号。
- 没手改 version；发版走 `cargo-release`。
