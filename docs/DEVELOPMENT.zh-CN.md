# upgit 开发手册

## 环境搭建

### rustup

```bash
git clone https://github.com/pluveto/upgit.git
cd upgit
```

确保安装了 [rustup](https://rustup.rs/)。

### C 语言工具链

若编译报错指向链接器或 `cc`，说明系统还缺少 C 语言工具链。
- Linux 需要 gcc；在 Debian 或 Ubuntu 上，安装 `build-essential` 软件包即可。
- macOS 需要 Xcode 命令行工具，执行 `xcode-select --install`。
- Windows 上的发布目标使用 Microsoft Visual C++ 工具链，需要安装 Visual Studio Build Tools，并勾选「使用 C++ 的桌面开发」工作负载。

### 剪贴板支持

在 Linux 上测试从剪贴板读取时，当前图形会话需要可用的剪贴板工具。

- 桌面环境若基于 X Window System，请安装 xclip；若基于 Wayland，请安装 wl-clipboard。

未安装时，与剪贴板相关的命令会失败，并提示需要安装的软件包。缺少这些程序不影响编译。

第一次编译请带上 `--locked`，让 Cargo 按照现有的 `Cargo.lock` 选取依赖版本。

```bash
cargo build --locked
cargo test --workspace --locked
```

你在本机执行的检查必须与 GitHub Actions 中名为 `CI` 的工作流相同，Clippy 的警告按错误处理。

Git 钩子位于 `.githooks/`。建议执行一次：

```bash
./scripts/install-git-hooks.sh
```

该脚本把 `core.hooksPath` 指到 `.githooks`。此后，提交和推送都会运行与上面相同的格式检查和 Clippy。

### 发布

发布使用 [cargo-release](https://github.com/crate-ci/cargo-release)。不必手工改版本字段。

```bash
cargo install cargo-release --locked
cargo release -V
```

推送标签需要写入本 GitHub 仓库的权限。

## 提交说明

提交标题遵循 [Conventional Commits](https://www.conventionalcommits.org/)。

## 发布

发布配置只允许在 `main` 上执行发布命令。不加 `--execute` 时只 dry run。

将预发布号加一：

```bash
cargo release beta
cargo release beta --execute --no-confirm
```

去掉预发布后缀，得到正式版本号：

```bash
cargo release release
cargo release release --execute --no-confirm
```

加上 `--execute` 之后，工具会改写版本字段和 `Cargo.lock`，提交，创建附注标签（`v` 加上版本号），并把 `main` 和该标签推到 `origin`。不要自己执行 `git tag` 来代替这些步骤。若漏改 `Cargo.lock`，持续集成的 `--locked` 检查会失败。

名称匹配 `v*` 的标签会启动发布工作流：先运行测试，再按矩阵编译各平台的压缩包，然后创建 GitHub Release。标签名里带 `-` 的版本会被标为预发布。

发布完成后打开 https://github.com/pluveto/upgit/releases ，核对标签名称、预发布开关、压缩包是否齐全。解压后的程序执行 `--version`，输出应等于标签去掉前缀 `v` 之后的版本号。
