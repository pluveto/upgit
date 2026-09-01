# Upgrading from 0.2

This page is for people who already run the Go 0.2 `upgit`. New users can ignore it and follow the [README](../README.md).

0.3 is not a drop-in. Flags, config paths, environment variables, and Qiniu credentials changed. HTTP image hosts are built in: add a TOML table instead of installing a JSONC file.

## Flags

| 0.2 | 0.3 |
| --- | --- |
| `--config-file` / `-c` | `--config` / `-c` |
| `--output-type` / `-o` | `--output` / `-o` |
| `--output-format` / `-f` | `--format` / `-f` |
| `:clipboard` as a file operand | `--clipboard` |
| `:clipboard-files` / `:clipboard-file` | `--clipboard-files` |

Examples:

```bash
# 0.2
upgit :clipboard --output-type clipboard --output-format markdown
upgit logo.png --config-file ./my.toml

# 0.3
upgit --clipboard --output clipboard --format markdown
upgit logo.png --config ./my.toml
```

AHK and Typora command lines that still pass `:clipboard` need the same change.

## Uploaders

There is no `extensions/` directory and no `upgit ext ls` / `ext add` / `ext my`. HTTP hosts (SM.MS, Imgur, CatBox, LSkyPro, …) ship in the binary.

- List hosts: `upgit uploaders`
- Enable one: copy its `[uploaders.*]` table from [`config.sample.toml`](../config.sample.toml) into your `config.toml` and set `default`, or pass `-u`

Do not drop a `.jsonc` file next to the binary and expect it to load.

## Qiniu

0.2 accepted a static upload token from the Qiniu dashboard. Those tokens expire.

0.3 signs a fresh token on every put from **AK/SK**:

```toml
default = "qiniu"

[uploaders.qiniu]
access_key = "..."
secret_key = "..."
bucket = "..."
public_base = "https://cdn.example.com/"
```

`prefix` is still accepted as an alias of `public_base`. Remove any old `token = "..."` field.

## Environment variables

Hardcoded `UPGIT_TOKEN`, `UPGIT_RENAME`, `UPGIT_USERNAME`, `UPGIT_REPO`, and `UPGIT_BRANCH` are gone.

Any config key can be set with `UPGIT_` and `__` for nesting:

```bash
# 0.2
export UPGIT_TOKEN=ghp_...
export UPGIT_USERNAME=your-user
export UPGIT_REPO=your-repo
export UPGIT_BRANCH=master

# 0.3
export UPGIT_DEFAULT=github
export UPGIT_UPLOADERS__GITHUB__PAT=ghp_...
export UPGIT_UPLOADERS__GITHUB__USERNAME=your-user
export UPGIT_UPLOADERS__GITHUB__REPO=your-repo
export UPGIT_UPLOADERS__GITHUB__BRANCH=main
export UPGIT_NAMING='{year}/{month}/{fname}_{unix}{ext}'
```

## Config file location

0.2 looked at `~/.upgit.config.toml`, `~/.config/upgitrc`, and `config.toml` / `upgit.toml` next to the binary (`--config-file` to override). It did **not** prefer the current directory.

0.3 search order:

1. `--config PATH`
2. `./config.toml`
3. Unix: `$XDG_CONFIG_HOME/upgit/config.toml` or `~/.config/upgit/config.toml`
4. Windows: `%APPDATA%\upgit\config.toml`, then `%USERPROFILE%`
5. `config.toml` next to the binary

`~/.upgit.config.toml` is **not** read. Copy that file to one of the paths above (or pass `--config`).

`upgit init` writes a GitHub-only `config.toml` to the platform config directory (Unix XDG / Windows `%APPDATA%\upgit`) unless you pass a path, and prints the full path.

## GitHub default branch

The packed / `init` template now uses `branch = "main"`. 0.2 samples often said `master`. The value must match the repository default branch, or the upload 404s / conflicts.

## What did not change in spirit

- `-t` / `--target-dir` still keeps the original filename under a remote directory
- `-s` / `--size-limit` still defaults to 5MiB; `0` is unlimited
- `-o clipboard` still copies the result (the flag is now `--output`)
- `-f markdown` is still `![{url_fname}]({url})`
- `[link]` URL rewrites are the old `replacements` table (`replacements` remains an alias)
