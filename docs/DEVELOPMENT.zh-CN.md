# upgit 开发手册

产品怎么用，看仓库根目录的 [README.md](../README.md) 和 [docs/README.zh-CN.md](README.zh-CN.md)。下面只写开发环境和发版。核心实现换掉之后，这份文件仍然适用：它不描述模块划分，只描述进仓库之后要遵守的约束，以及机器上要装什么、发版按哪条命令走。

默认分支是 `main`。日常开发和打 tag 都在这根线上。更早的一条历史线在 `v0.2-main`，不在那上面做新工作。

版本号只写在仓库约定的那一处 workspace 版本字段里，由发版命令改。不要在多个清单里各抄一个数字。软件发给用户的方式是 GitHub Release 上的压缩包，不是语言包注册中心。

## 三巨头

设计和评审跟 Eric Evans、John Ousterhout、Alan Kay 走。改对外行为或核心抽象之前，用他们的标准看一遍。讨论方案时三人都要到场，互相质疑至少一轮，话落到文件路径、函数名、不变量上。看不清就写「无法核实」。

Evans 的书是《领域驱动设计》。Ousterhout 的书是《软件设计哲学》。Kay 没有对应的那一本专著，依据是 *The Early History of Smalltalk* 和他公开谈 OOP 的文字。

Evans 要求用户、文档、代码共用一套词。用户说「上传器」「配置」「直链」，源码和报错就用对应的那些名字。帮助文本、init 打到屏幕上的字、发布包里配置文件的注释，不要写内部模块名。有人把失败写成实现细节再贴一段示例配置，等于逼用户学两套语言。领域层不依赖 HTTP 客户端，也不依赖命令行解析库。命名规则、URL 改写、把文件送走，是三个对象上的事，不要为了省事在某一家图床的客户端里偷偷改名。远端返回的 JSON 或 XML 不准原样印给用户。错误写发生了什么，下一行写该改配置里的哪一项。

Ousterhout 把复杂度定义成：下一次改动变难了。接口短、实现厚。调用方不要自己把「取名、上传、改写地址」拆开再拼一遍。缺的密钥在读配置时就失败，不要等请求已经打到网上，再把一页 HTML 当成 JSON 解析。占位符密钥（还带着 `PASTE_` 或 `...`）按没填处理。子命令必须交给参数解析器；在 `main` 里偷看 `argv[1]` 会把 `--help` 收成文件名。测试调用将要上线的那个函数。在测试里再写一遍算法去对答案，两边一起错也会绿。文档写了而代码没接到，不算做完。

CI 和 git hook 用同一组检查。本仓库里 Clippy 的 warning 就是 error。README 里不要写会死掉的版本号。用户问版本，看程序自己的 `--version`，或 git tag。

Kay 要求上传器是带着自己配置的对象，只收「把这个文件放到这个键上」这一条消息。不要拆成全局配置结构体加一排按名字分支的函数。换图床是换成另一个对象，不是在总线上再加一个分支。能用一份请求模板说清的 HTTP 图床，不要为每一家再抄一套客户端。不要把上传器收成闭包来显得函数式：状态会进捕获列表，测试夹具失去边界。

讨论方案时把选项写清楚，写若选 A 就要放弃 B。声称做完了、可以打 tag 时拿代码当证据。两人通过、一人不通过，听更严的那个。主持判断的人和写代码的人不要挤在同一回合。

## 环境

你本机跑过的检查，必须和 GitHub Actions 里名为 `CI` 的那份工作流相同。少一条，就会本地绿、GitHub 红。这份工作流在 `main` 的 push 和 pull request 上跑，操作系统包括 Ubuntu、Windows、macOS。

```bash
git clone https://github.com/pluveto/upgit.git
cd upgit
```

`git status` 应显示 `main`。

根目录的 `rust-toolchain.toml` 指定 stable，以及 rustfmt、clippy。先装 [rustup](https://rustup.rs/)。进入这个目录之后，rustup 会按文件下载工具链。跑 `rustc --version`、`cargo --version`，再跑 `rustup component list --installed`。输出里要有 rustfmt 和 clippy，缺了就 `rustup component add rustfmt clippy`。

从中国大陆拉 crates.io 经常超时。可以先 `export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`。仍慢就按你常用的镜像站文档改 cargo 的配置文件。镜像只影响下载，不改变 `--locked` 的含义。

部分依赖会编 C 代码。Linux 上要有 gcc（Debian/Ubuntu 的包名是 `build-essential`）。macOS 上要有 Command Line Tools：`xcode-select --install`。Windows 上发布目标是 MSVC，需要 Visual Studio Build Tools，工作负载选「使用 C++ 的桌面开发」。缺链接器时，报错往往看起来像 Rust 的，根因是系统工具链。

Linux 上测剪贴板需要运行时的剪贴板程序：X11 用 xclip，Wayland 用 wl-clipboard。没装时相关子命令会失败并写出要装什么。这和能不能编译无关。

第一次编译带 `--locked`，让 cargo 按现有 lock 文件取版本。CI 也带这个旗标。忘了带，cargo 可能改 lock，diff 里出现一堆无关升级。只有确实要升依赖时才去掉 `--locked`，并把新的 lock 文件一并提交。

```bash
cargo build --locked
cargo test --workspace --locked
```

能跑起来之后，再确认格式和 Clippy。本仓库把 `RUSTFLAGS=-D warnings` 开在 CI 上，Clippy 的 warning 按 error 处理：

```bash
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --locked
```

钩子文件在 `.githooks/`，clone 下来不会自动启用。每个新的工作副本执行一次：

```bash
./scripts/install-git-hooks.sh
```

它把 `core.hooksPath` 指到 `.githooks`。之后，提交和推送都会跑和上面相同的 fmt 检查与 Clippy。fmt 的 `--check` 只报告，不会替你改文件。先 `cargo fmt --all`，再 add、commit。不要用 `--no-verify`：钩子挡住的东西，CI 同样会挡，只是更晚。

你在 Linux 或 macOS 上提交时，钩子编译的是当前目标。只在 Windows 配置下才会编进来、却从未调用的函数，这边的 Clippy 看不见。Windows 上的 CI 会把它当成 dead_code。动了按操作系统分开的代码，不能只靠本机钩子签字。

若本机 git 开了 `commit.gpgsign=true`，而当前会话没有可用的 pinentry，`git commit` 会报 `Inappropriate ioctl for device`。发版程序内部也会调 `git commit`，同一处失败。它自己的配置里关掉签名，挡不住 git 的全局开关。在这个仓库的克隆上：

```bash
git config --local commit.gpgsign false
git config --show-origin commit.gpgsign
```

第二条用来确认生效的是 local。

发版用 [cargo-release](https://github.com/crate-ci/cargo-release)，不要手改版本字段。

```bash
cargo install cargo-release --locked
cargo release -V
```

推 tag 需要写这个 GitHub 仓库的权限。`gh auth status` 里当前账号必须对。曾经用错账号推送，GitHub 返回 403。

## 提交说明

提交标题用 [Conventional Commits](https://www.conventionalcommits.org/)。用户能察觉的新能力用 `feat:`。缺陷用 `fix:`。工作流用 `ci:`。发版程序自己写的版本提交是 `chore: bump version to {{version}}`。文档用 `docs:`。不改变对外行为的结构移动用 `refactor:`。

标题一行说做成了什么。需要理由时，空一行再写正文。不要把改动顺序写进标题。

## 发版

发版配置文件只允许在 `main` 上发。不加 `--execute` 时只打印将要做的事，不改工作区。

预发布号往前加一：

```bash
cargo release beta
cargo release beta --execute --no-confirm
```

去掉预发布后缀，得到正式号：

```bash
cargo release release
cargo release release --execute --no-confirm
```

正式版必须写成 `cargo release release`。只写 `cargo release --execute` 时，工具可能认为当前版本对应的 tag 已经存在，直接退出。

`--execute` 之后它改版本字段和 lock 文件，提交，打 annotated tag（`v` 加版本号），把 `main` 和这个 tag 推到 `origin`。不要自己 `git tag` 来省步骤。漏改 lock 时，CI 的 `--locked` 会失败。

名字匹配 `v*` 的 tag 会启动发版工作流：先跑测试，再按矩阵编各平台的压缩包，然后创建 GitHub Release。tag 里带 `-` 的标成 prerelease。在普通分支上手动触发工作流不会发 Release，避免用分支名当 tag。

发完打开 https://github.com/pluveto/upgit/releases ，看 tag 名字、prerelease 开关、压缩包是否齐全。解出来的程序跑 `--version`，应等于 tag 去掉前缀 `v`。
