# ![upgit](https://cdn.jsdelivr.net/gh/pluveto/upgit/logo.png)



*Upgit* 可以快捷地将文件上传到 Github 仓库并得到其直链。

可作为 [Typora](https://typora.io/) 的自定义上传器使用。

## 特点

+ 支持多平台，包括 Linux、Windows macOS
+ 不限制文件类型
+ 支持从**剪贴板上传**
+ 自定义**自动重命名**规则（包括路径）
+ 可通过替换规则实现**CDN**加速
+ 可通过**环境变量**配置
+ 将 URL 输出到标准输出/**剪贴板**，支持 Markdown 格式

## 开始使用

### 下载

从[Release](https://github.com/pluveto/upgit/releases) 下载.

>如果不知道下载哪一个：
>
> + 对于大多数 Windows用户，请选择 `upgit_win_amd64.exe`
> + 对于大多数 macOS用户，请选择 `upgit_macOS_arm64`

下载后将其重命名为`upgit`（对于Windows用户，`upgit.exe`），保存到某处。若要从任何地方访问它，请将其目录添加到 `PATH` 环境变量中。

**提醒：此程序不自动更新。如果你需要保持更新，可以点右上角的 ⭐star 收藏。

### 配置

在程序的同一目录创建 `config.toml` 文件，内容按照[此示例配置文件](https://github.com/pluveto/upgit/blob/main/config.sample.toml) 填写即可.

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

有关更多帮助，请键入“-h”参数


```shell
./upgit -h

Upload anything to git and then get its link.
For more information: https://github.com/pluveto/upgit

Usage: upgit.exe [--target-dir TARGET-DIR] [--verbose] [--size-limit SIZE-LIMIT] [--wait] [--clean] [--raw] [--output-type OUTPUT-TYPE] FILE [FILE ...]

Positional arguments:
  FILE                   local file path to upload. :clipboard for uploading clipboard image
Options:
  --target-dir TARGET-DIR, -t TARGET-DIR
                         upload file with original name to given directory. if not set, will use renaming rules
  --verbose, -V          when set, output more details to help developers
  --size-limit SIZE-LIMIT, -s SIZE-LIMIT
                         in bytes. overwrite default size limit (5MiB). 0 means no limit
  --wait, -w             when set, not exit after upload, util user press any key
  --clean, -c            when set, remove local file after upload
  --raw, -r              when set, output non-replaced raw url
  --output-type OUTPUT-TYPE, -o OUTPUT-TYPE
                         output type, support stdout(default), clipboard, clipboard-markdown [default: stdout]
  --help, -h             display this help and exit
```



### 配合 Typora 使用

> 假设 *upgit* 程序保存在`“C:\repo\upgit\upgit.exe`。

选择 *文件 > 首选项*

![image-20220128204217802](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373863.png)

转到 *Image*。选择*自定义命令*作为*图像上传器*。

在*命令*文本框中输入*upgit* 程序位置。

> 你可以点击*测试上传*按钮来确保它工作正常。

![image-20220128204418723](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373868.png)

然后就可以使用了。

### 上传剪贴板图像



使用  `:clipboard`  占位符放置剪贴板图像。（仅支持**png**格式）

```shell
./upgit :clipboard
```

截图快捷键：

- 在 macOS 上，使用 `Ctrl+Shift+Cmd+4`
- 在 Linux/Ubuntu 上，使用 `Ctrl+Shift+PrintScreen`
- 在 Windows 上，使用 `Shift+Win+s`



### 将 URL 保存到剪贴板

使用参数 `--output-type clipboard-markdown`:


```shell
./upgit logo.png --output-type clipboard-markdown
# or ./upgit :clipboard --o clipboard-markdown
```

然后会在剪贴板上得到一个 Markdown 图片链接，比如：

```md
![logo.png]（！https://cdn.jsdelivr.net/gh/pluveto/upgit/logo.png)
```

### AHK 的最佳实践

对于 Windows 用户：

1. 安装AHK
2. 创建这个脚本 `upload_clipboard.ahk` 并运行：
   ```ahk
   # Ctrl+F9 As shortcut
   ^F9::
   RunWait, explorer ms-screenclip:
   # use your own *upgit* program path
   RunWait, "upgit.exe" :clipboard --output-type clipboard-markdown
   return
   ```
3. 然后按 <kbd>Ctrl</kbd><kbd>F9</kbd>自动截图、上传并将其链接复制到剪贴板！


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
| -- `{unix_ts}`        | -- 以秒计的 Unix 时间戳，如 `1643373370`. |
| --- `{ext}`           | -- 扩展名，如 `.png`，若文件无扩展名，则为空串 |
| -- `{file_name}`      | -- 原始文件名，如 `logo` （不含扩展名） |
| -- `{file_name_hash}` | -- `{file_name}`的 MD5 散列值               |

这是一个简单的示例配置文件：

```toml
pat = "ghp_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
rename = "{year}/{month}/upgit_{year}{month}{day}_{unix_ts}{ext}"
repo = "repo-name"
username = "username"
```
