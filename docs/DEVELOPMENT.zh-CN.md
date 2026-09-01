# upgit 开发手册

产品用法在仓库根目录的 [README.md](../README.md) 和 [docs/README.zh-CN.md](README.zh-CN.md)。下面只写怎么在这棵树上开发，以及怎么把一版软件交到 GitHub Releases。

默认分支叫 `next`。clone 下来就在这根线上。发版也只允许从这里推 tag。

版本号只有一处：根目录 `Cargo.toml` 里 `[workspace.package]` 的 `version`。三个成员 crate 写 `version.workspace = true`，彼此用 `path` 依赖，不要再抄一遍数字。`publish = false`，我们不往 crates.io 上传。用户拿到的是 Release 页面上的 zip。

`crates/upgit-core` 里是领域对象。一个本地文件进门之后叫 `Artifact`，远端路径叫 `ObjectKey`，谁来命名由 `KeyPolicy` 决定，谁来改写展示用的 URL 由 `LinkPolicy` 决定，真正把字节送走的是实现了 `Uploader` 的对象。`Publisher` 按顺序问这几位要结果。报给用户的失败形状是 `UploadError`。

`crates/upgit-uploaders` 认识各家图床。它把一份 TOML 配成具体的上传器对象，塞进 `Registry`。HTTP 图床的请求模板在 `recipes/`，编译时编进二进制。`HostCatalog` 是给 `--help` 和 `upgit uploaders` 看的名单。

`crates/upgit` 是命令行。clap 解析参数，`Intake` 从磁盘或剪贴板取出 `Artifact`，`App` 找到上传器并调用 `Publisher`，`Emitter` 把 URL 打到 stdout 或剪贴板。`upgit init` 是 clap 子命令，写一份 GitHub 用的 `config.toml`。

发布 zip 用的模板是 `config.github.toml`。仓库里的 `config.sample.toml` 是完整图床目录，打 zip 时不要带上它。钩子在 `.githooks/`，工作流在 `.github/workflows/`，发版工具的配置在 `release.toml`，Rust 版本钉在 `rust-toolchain.toml`。

## 三巨头

设计和评审跟 Eric Evans、John Ousterhout、Alan Kay 走。这不是装饰。改 `Uploader`、改错误文案、改测试门槛之前，先用他们的标准看一遍。讨论方案时三人都要到场，互相质疑至少一轮，话要落到文件路径、函数名、不变量上。看不清就写「无法核实」，不要编。

Evans 的书是《领域驱动设计》。Ousterhout 的书是《软件设计哲学》。Kay 没有对应的那一本专著，依据是 *The Early History of Smalltalk* 和他公开谈 OOP 的文字。

### Evans

用户说「上传器」，代码里的类型就叫 `Uploader`。用户说「配置」，解析结果叫 `AppConfig`。用户要的那条能打开的地址，上传刚结束时叫 `Locator`，经过 `LinkPolicy` 之后叫 `PublicUrl`。这套词要写进文档和错误信息。`--help`、`init` 打到屏幕上的字、zip 里 `config.toml` 的注释，都不要出现 `registry`、`recipe id`、`first-class`、`JSONC`、`extensions/`。那些是实现细节。有人把未知上传器的报错写成「没有 extensions 文件夹」再贴一段七牛 TOML，等于逼用户学两套语言。正确做法是：`Registry::get` 只报已经装进表里的 id；完整能用的 id 由 `HostCatalog` 提供；提示去改 `config.toml` 的 `default` 或传 `--uploader`。

`upgit-core` 不准认识 ureq，也不准认识 clap。它只知道文件、键、上传这条消息。把 HTTP 客户端或参数解析塞进 core，以后换客户端就要改领域层。`upgit-uploaders` 负责读 TOML、认出这是 GitHub 还是一份 HTTP 配方，然后 `new` 出对象、`register` 进表。`upgit` 负责从用户那儿收文件，把 URL 打回去。

命名发生在 `KeyPolicy`。某个 Uploader 自己改远程文件名，别的 Uploader 就对不齐，`-t` 也就没地方接。CDN 替换发生在 `LinkPolicy`。`Uploader::upload` 的参数是 `&Artifact` 和 `&ObjectKey`，返回 `Locator`。GitHub 的 Contents API 把 JSON 倒给用户，是把基础设施的形状泄漏出了领域。`UploadError` 的 `what` 写发生了什么，`hint` 写下一步改配置的哪一项，`status` 在有 HTTP 码时带上。`documentation_url` 这类字段不准出现在 `to_string()` 里。

### Ousterhout

复杂度的意思是：下一次改动变难了。`Publisher::publish` 已经把「取键、upload、改写 URL」收在一个调用里。调用方再自己 `apply` 一遍 `KeyPolicy` 再 `upload`，就是把实现细节摊开，以后改顺序要改很多处。

缺字段要在装配置时失败。SM.MS 的配方里有 `{config.token}`，空表 `[uploaders.smms]` 必须在 `install_into` 报缺少 `token`。若放行，请求会打到 sm.ms，把 HTML 当 JSON 解析，用户看到 `invalid json response`。`naming` 里有 `{hmac}` 而 `hmac_key` 为空，必须在 `AppConfig::namer()` 失败。以前 `interpolate` 会把 `{hmac}` 原样留在对象键里，文件上传「成功」，路径却是字面量。zip 里的 `pat = "PASTE_YOUR_TOKEN"` 含 `PASTE_` 或 `...`，按缺字段处理，否则 GitHub 返回 401，用户以为自己填过了。

`init` 必须走 clap 子命令。曾经在 `main` 里判断 `args[1] == "init"`，把 `args[2]` 当路径，于是 `upgit init --help` 在当前目录写出一个名叫 `--help` 的文件。那是接口把非法输入收成了合法路径。

测试要调用将要上线的那个函数。在测试里再写一遍 HMAC 再和实现比，两边一起错也会绿。文档写了「支持 `-t`」而 CLI 从未调用 `KeyPolicy::keep_original_in`，这件事不算做完。把失败改成注释里的「已知问题」也不算做完。

CI 和 git hook 用同一组命令：`cargo fmt --all -- --check`；`RUSTFLAGS=-D warnings cargo clippy --workspace --all-targets --locked`；`cargo test --workspace --locked`。Clippy 的 warning 在这里就是 error。README 里不要出现 `v0.3.0-alpha.2` 这种会过期的 tag。用户问版本，看 `upgit --version` 或 git tag。

### Kay

`GithubUploader` 自己带着 pat、username、repo、branch。你对它只发 `upload`。不要把这些字段拆到一个全局 `Config` 再写 `fn github_put(cfg, file)`。`QiniuUploader` 每次 put 自己用 AK/SK 签发 token。`HttpRecipeUploader` 拿着配方和一张 `config` 表，自己做插值和 HTTP。

一次上传的顺序是：`Intake` 给出 `Artifact`；`Publisher` 向 `KeyPolicy` 要 `ObjectKey`；把这两样发给 `Uploader`；拿到 `Locator` 再交给 `LinkPolicy`，得到给用户看的 URL。中间不要去读 `GithubUploader` 的私有字段来拼 raw.githubusercontent.com。

配置表名叫 `smms` 且 catalog 里有这个 id，就不必写 `type = "http"`。换图床是 `Registry` 里换成另一个对象。按 id 写 `match "github" => { ureq::put(...) }` 会让每家图床都把签名、重试、错误翻译写进同一条总线，无法替换。

HTTP 图床用 `recipes/*.toml` 描述请求。为每一家再抄一套客户端，是为特例发明特例。签名、分块、和 Contents API 强相关的存储，才各自做一个 Uploader 类型。

不要把 `Uploader` 收成闭包来「函数式一点」。闭包把状态藏进捕获列表，错误翻译和测试夹具都失去对象边界。

讨论方案时用 deliberate：把选项写清楚，写若选 A 就要放弃 B。声称做完了、可以打 tag 时用 audit：拿代码当证据。两人通过、一人不通过，听更严的那个。主持判断的人和写代码的人不要是同一回合。

## 环境

你本机跑过的检查，必须和 `.github/workflows/ci.yml` 里 `CI` 这一份 job 相同。少一条，就会本地绿、GitHub 红。CI 在 `next` 的 push 和 pull request 上跑，操作系统是 ubuntu-latest、windows-latest、macos-latest。

```bash
git clone https://github.com/pluveto/upgit.git
cd upgit
```

此时 `git status` 应显示 `next`。

根目录的 `rust-toolchain.toml` 写了 `channel = "stable"`，以及 `components = ["rustfmt", "clippy"]`。先装 [rustup](https://rustup.rs/)。进入这个目录之后，rustup 会按文件下载对应的 stable，并把 rustfmt、clippy 装上。跑 `rustc --version`、`cargo --version`。再跑 `rustup component list --installed`，输出里要有 rustfmt 和 clippy。缺了就 `rustup component add rustfmt clippy`。

从中国大陆拉 crates.io 经常超时。可以先 `export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`。仍慢就按 rsproxy 或你常用的镜像站文档改 `config.toml`。镜像只影响下载，不改变 `--locked` 的含义。

部分 crate 会编 C 代码。Linux 上要有 gcc（Debian/Ubuntu 的包名是 `build-essential`）。macOS 上要有 Command Line Tools：`xcode-select --install`。Windows 上本仓库的发布目标是 `*-pc-windows-msvc`，需要 Visual Studio Build Tools，工作负载选「使用 C++ 的桌面开发」。缺 MSVC 时 `ring` 这类依赖会在编译期失败，报错看起来像 Rust 的，根因是链接器。

`--clipboard` 在 Linux 上还依赖运行时的剪贴板程序。X11 用 xclip，Wayland 用 wl-clipboard。没装时 `upgit --clipboard` 会失败，并在错误里写出要装哪个。这和能不能 `cargo build` 无关。

第一次编译：

```bash
cargo build -p upgit --locked
cargo run -p upgit --locked -- --version
```

`--locked` 让 cargo 严格按现有 `Cargo.lock` 取版本。CI 带这个旗标。你若忘了带，cargo 可能悄悄改 lock，diff 里出现一堆无关升级。只有在你确实要升依赖时才去掉 `--locked`，并把新的 `Cargo.lock` 一并提交。

`cargo run -p upgit --locked -- --help` 应打印用法，底部有上传器名单和 `https://github.com/pluveto/upgit`。不带参数的 `upgit` 也打印帮助，进程退出码是 2。它不得先去读 `config.toml` 再报「没有配置上传器」。`cargo run -p upgit --locked -- uploaders` 应列出 `github`、`smms` 这些 id。`cargo run -p upgit --locked -- init --help` 必须是 clap 的帮助文本，当前目录不得出现名为 `--help` 的文件。

钩子文件在 `.githooks/`，clone 下来不会自动启用。每个新的工作副本执行一次：

```bash
./scripts/install-git-hooks.sh
```

脚本里是 `git config core.hooksPath .githooks`。之后，往暂存区放 `.rs`、`.toml` 或 `Cargo.lock` 再 `git commit`，pre-commit 会跑 `cargo fmt --all -- --check`，以及 `RUSTFLAGS=-D warnings` 下的 `cargo clippy --workspace --all-targets --locked`。`git push` 时 pre-push 无条件再跑同一组。fmt 的 `--check` 只报告，不会替你改文件。先 `cargo fmt --all`，再 add、commit。

不要用 `--no-verify`。钩子挡住的东西，CI 同样会挡，只是更晚、且挡住所有人的合并。

你在 Linux 或 macOS 上提交时，钩子编译的是当前目标。`#[cfg(windows)]` 里多出来、从未调用的函数，这边的 clippy 看不见。`windows-latest` 上 `RUSTFLAGS=-D warnings` 会把它当成 dead_code 判失败。`decode_path` 和 `from_hex` 只给非 Windows 的剪贴板文本路径用，必须标 `#[cfg(not(windows))]`。动了 `source.rs` 里 Windows 分支，不能只靠本机钩子签字。

日常在提交前自己跑一遍（和 CI 相同）：

```bash
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
```

若本机 git 开了 `commit.gpgsign=true`，而当前会话没有可用的 pinentry（SSH 进来的环境、CI 里的 agent、部分 IDE 集成终端），`git commit` 会报 `gpg: signing failed: Inappropriate ioctl for device`。`cargo-release` 内部调用 `git commit`，同一处失败。`release.toml` 里的 `sign-commit = false` 只关掉 cargo-release 自己的签名开关，挡不住 git 全局配置。在这个仓库的克隆上执行：

```bash
git config --local commit.gpgsign false
git config --show-origin commit.gpgsign
```

第二条用来确认生效的是 local 而不是 global。

发版还要装 [cargo-release](https://github.com/crate-ci/cargo-release)：

```bash
cargo install cargo-release --locked
cargo release -V
```

推 tag 需要写这个 GitHub 仓库的权限。`gh auth status` 里当前账号必须是有 `repo` 权限的那一个。曾经用错账号推 `next`，GitHub 返回 403。

## 提交说明

提交标题用 [Conventional Commits](https://www.conventionalcommits.org/)。用户能察觉的新能力用 `feat:`。缺陷用 `fix:`。工作流用 `ci:`。cargo-release 自己写的版本提交是 `chore: bump version to {{version}}`。文档用 `docs:`。不改变对外行为的结构移动用 `refactor:`。

标题一行说做成了什么。需要理由时，空一行再写正文。不要把「先改了 A 再改了 B」写进标题。

## 发版

`release.toml` 里 `allow-branch = ["next"]`。在别的分支上执行 cargo-release 会被拒绝。不加 `--execute` 时它只打印将要做的事，不改工作区。

预发布号往前加一（`0.3.0-beta.3` 变成 `0.3.0-beta.4`）：

```bash
cargo release beta
cargo release beta --execute --no-confirm
```

去掉 `-beta.N`，得到正式号 `0.3.0`：

```bash
cargo release release
cargo release release --execute --no-confirm
```

第二条命令必须写成 `cargo release release`。只写 `cargo release --execute` 时，cargo-release 可能认为当前版本对应的 tag 已经存在，直接退出。正式版发过一次，就是这样撞上 `v0.3.0-beta.3 already exists` 的。

`--execute` 之后它改根 `Cargo.toml` 的 `version`，改 `Cargo.lock` 里三处相同的数字，提交 `chore: bump version to …`，打 annotated tag `v` 加版本号，把 `next` 和这个 tag 推到 `origin`。不要自己改三个 crate 目录里的 version，也不要手打 `git tag` 来「节省步骤」。漏改 lockfile 时，CI 的 `--locked` 会失败。

tag 名字匹配 `v*` 时，`.github/workflows/release.yml` 开始跑。先在 ubuntu-latest 上 `cargo test --workspace --locked`。通过后按矩阵编 zip：

- `upgit_linux_386.zip`、`upgit_linux_amd64.zip`、`upgit_linux_arm.zip`、`upgit_linux_arm64.zip`
- `upgit_win_386.zip`、`upgit_win_amd64.zip`、`upgit_win_arm64.zip`
- `upgit_macos_amd64.zip`、`upgit_macos_arm64.zip`

每个 zip 里有一份名为 `upgit` 的二进制（Windows 上是 `upgit.exe`）、一份从 `config.github.toml` 拷出来的 `config.toml`、以及 `recipes/` 目录。Unix 上名为 `upgit` 的文件在 zip 里的 `external_attr` 是 `0o755`，否则解压后不可执行。断言会拒绝任何文件名里带 `config.sample` 的条目。

创建 GitHub Release 时，若 tag 含 `-`（如 `v0.3.0-beta.4`），`prerelease` 为 true；`v0.3.0` 为 false。`workflow_dispatch` 若跑在普通分支上，`github.ref_type` 不是 `tag`，publish job 被 `if` 跳过，避免用分支名当 tag 发一版空的 release。

发完打开 https://github.com/pluveto/upgit/releases ，看 tag 名字、prerelease 开关、上面九个 zip 是否都在。把解出来的二进制跑 `--version`，应等于 tag 去掉前缀 `v`。

README、`--help`、`init` 打印的那几行、zip 里 `config.toml` 的注释、Release 说明正文里，不要出现 `JSONC`、`extensions/`、`registry`、`first-class`、`recipe id`、`20 bundled`。
