# ![upgit](https://github.com/pluveto/upgit/blob/main/logo.png?raw=true)

*Upgit* helps you simply upload any file to your Github repository and then get a raw URL for it.

This is also useful with [Typora](https://typora.io/) as an image uploader.

## Get started

### Download

Download it from [Release](https://github.com/pluveto/upgit/releases).

> If you have no idea which to download:
>
> + For most Windows users, choose `upgit_win_amd64.exe`
> + For most macOS users, choose `upgit_macos_amd64`

Rename it to `upgit` (For Windows users, `upgit.exe`), save it to somewhere you like.

**Warning:** this program doesn't contain an auto-updater. If you need to keep updated, just give *upgit* a ⭐star.

### Config

Create `config.toml` in the same directory of *upgit*, and fill it in following [this sample config file](https://github.com/pluveto/upgit/blob/main/config.sample.toml).

### Use it

To upload file `logo.png` with rename rules, execute:

```shell
./upgit logo.png
# for windows: .\upgit.exe logo.png
```

Then you'll see a link to `logo.png`.

To upload file `logo.png` to remote folder `/my_images/demo`, execute:

```shell
./upgit logo.png -t /my_images/demo
# for Windows: .\upgit.exe logo.png -t /my_images/demo
```

---

For more help, type `-h` argument

```
./upgit -h

Upload anything to git and then get its link.
For more information: https://github.com/pluveto/upgit

Usage: upgit.exe [--target-dir TARGET-DIR] [--verbose] [--size-limit SIZE-LIMIT] FILE [FILE ...]

Positional arguments:
  FILE

Options:
  --target-dir TARGET-DIR, -t TARGET-DIR
                         upload file with original name to given directory. if not set, will use renaming rules
  --verbose, -V          will output more details to help developers
  --size-limit SIZE-LIMIT, -s SIZE-LIMIT
                         in bytes. overwrite default size limit (5MiB). 0 means no limit
  --help, -h             display this help and exit
```

### Use it for Typora

> Assuming your *upgit* program is saved at `"C:\repo\upgit\upgit.exe"`.

Select *File > Preferences...*

![image-20220128204217802](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373863.png)

Move to *Image*. Choose *Custom Command* as your *Image Uploader*.

Input *upgit* program location into *Command* textbox.

Now enjoy it.

> You can click *Test Uploader* button to make sure it works.

![image-20220128204418723](https://cdn.jsdelivr.net/gh/pluveto/0images@master/2022/01/upgit_20220128_1643373868.png)



## Config Instructions



| Key                   | Desc                                                         |
| --------------------- | ------------------------------------------------------------ |
| username              | Your Github username, like `pluveto`                         |
| repo                  | Your Github repository name, like `upgit`                    |
| branch                | The branch for saving files, like `master` or `main`         |
| pat                   | Personal Access Token. Visit [GitHub Docs](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token) for more info |
| rename                | Renaming rule. Path separator `/` will create directories if not exists. Supporting: |
| -- `{year}`           | -- Year like `2006`                                          |
| -- `{month}`          | -- Month like `01`                                           |
| -- `{day}`            | -- Day like `02`                                             |
| -- `{unix_ts}`        | -- Unix timestamp in second. Like `1643373370`.              |
| --- `{ext}`           | -- Extension like `.png`, and empty when the original file has no extension |
| -- `{file_name}`      | -- Original file base name like `demo` (without extension)   |
| -- `{file_name_hash}` | -- MD5 Hash in hex of `{file_name}`                          |

Here is a simplist sample config file:

```toml
pat = "ghp_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
rename = "{year}/{month}/upgit_{year}{month}{day}_{unix_ts}{ext}"
repo = "repo-name"
username = "username"
```

## Todo

[x] Upload to specific folder

[ ] Upload and get raw URL
