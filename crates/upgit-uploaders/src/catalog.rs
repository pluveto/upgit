use std::path::{Path, PathBuf};

use crate::recipe::{HttpRecipe, RecipeError};

/// One built-in or HTTP-recipe host, for help and listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Host {
    pub id: &'static str,
    pub title: &'static str,
}

/// Built-in uploaders first, then HTTP recipes in [`RecipeCatalog`] order.
pub struct HostCatalog;

impl HostCatalog {
    pub fn all() -> &'static [Host] {
        &[
            Host {
                id: "github",
                title: "GitHub",
            },
            Host {
                id: "gitlab",
                title: "GitLab",
            },
            Host {
                id: "s3",
                title: "Amazon S3 (MinIO / Cloudflare R2 / Backblaze / Wasabi / DigitalOcean Spaces / Ceph / Flexify.IO / IBM Cloud Object Storage)",
            },
            Host {
                id: "aliyunoss",
                title: "Aliyun OSS",
            },
            Host {
                id: "qcloudcos",
                title: "Tencent Cloud COS",
            },
            Host {
                id: "upyun",
                title: "Upyun",
            },
            Host {
                id: "qiniu",
                title: "Qiniu Kodo",
            },
            Host {
                id: "smms",
                title: "SM.MS",
            },
            Host {
                id: "imgur",
                title: "Imgur",
            },
            Host {
                id: "catbox",
                title: "Catbox",
            },
            Host {
                id: "cloudinary",
                title: "Cloudinary",
            },
            Host {
                id: "easyimage",
                title: "EasyImage",
            },
            Host {
                id: "lskypro",
                title: "Lsky Pro",
            },
            Host {
                id: "lskypro2",
                title: "Lsky Pro v2",
            },
            Host {
                id: "hello",
                title: "Helloimg",
            },
            Host {
                id: "niupic",
                title: "Niupic",
            },
            Host {
                id: "imgurlorg",
                title: "ImgURL.org",
            },
            Host {
                id: "imgbb",
                title: "ImgBB",
            },
            Host {
                id: "chevereto",
                title: "Chevereto",
            },
            Host {
                id: "gitee",
                title: "Gitee",
            },
            Host {
                id: "dalexni",
                title: "DALEXNI",
            },
            Host {
                id: "imgtg",
                title: "img.tg",
            },
            Host {
                id: "juejin",
                title: "Juejin",
            },
            Host {
                id: "moetu",
                title: "Moetu",
            },
            Host {
                id: "netease",
                title: "NetEase",
            },
            Host {
                id: "sougou",
                title: "Sogou",
            },
            Host {
                id: "upload_cc",
                title: "upload.cc",
            },
        ]
    }

    pub fn ids() -> impl Iterator<Item = &'static str> {
        Self::all().iter().map(|host| host.id)
    }

    pub fn id_width() -> usize {
        Self::all()
            .iter()
            .map(|host| host.id.len())
            .max()
            .unwrap_or(8)
    }
}

/// Bundled HTTP recipes: on disk next to the binary, then compiled-in copies.
pub struct RecipeCatalog;

impl RecipeCatalog {
    pub fn embedded() -> &'static [(&'static str, &'static str)] {
        &[
            ("smms", include_str!("../../../recipes/smms.toml")),
            ("imgur", include_str!("../../../recipes/imgur.toml")),
            ("catbox", include_str!("../../../recipes/catbox.toml")),
            (
                "cloudinary",
                include_str!("../../../recipes/cloudinary.toml"),
            ),
            ("easyimage", include_str!("../../../recipes/easyimage.toml")),
            ("lskypro", include_str!("../../../recipes/lskypro.toml")),
            ("lskypro2", include_str!("../../../recipes/lskypro2.toml")),
            ("hello", include_str!("../../../recipes/hello.toml")),
            ("niupic", include_str!("../../../recipes/niupic.toml")),
            ("imgurlorg", include_str!("../../../recipes/imgurlorg.toml")),
            ("imgbb", include_str!("../../../recipes/imgbb.toml")),
            ("chevereto", include_str!("../../../recipes/chevereto.toml")),
            ("gitee", include_str!("../../../recipes/gitee.toml")),
            ("dalexni", include_str!("../../../recipes/dalexni.toml")),
            ("imgtg", include_str!("../../../recipes/imgtg.toml")),
            ("juejin", include_str!("../../../recipes/juejin.toml")),
            ("moetu", include_str!("../../../recipes/moetu.toml")),
            ("netease", include_str!("../../../recipes/netease.toml")),
            ("sougou", include_str!("../../../recipes/sougou.toml")),
            ("upload_cc", include_str!("../../../recipes/upload_cc.toml")),
        ]
    }

    pub fn ids() -> impl Iterator<Item = &'static str> {
        Self::embedded().iter().map(|(id, _)| *id)
    }

    pub fn contains(id: &str) -> bool {
        Self::embedded().iter().any(|(known, _)| *known == id)
    }

    pub fn load(id: &str) -> Result<HttpRecipe, RecipeError> {
        if let Some(text) = Self::read_text(id) {
            return HttpRecipe::from_toml(&text);
        }
        Err(RecipeError::Message(format!(
            "unknown recipe `{id}` (bundled: {})",
            Self::ids().collect::<Vec<_>>().join(", ")
        )))
    }

    pub fn extract_to(dir: &Path) -> std::io::Result<usize> {
        std::fs::create_dir_all(dir)?;
        let mut n = 0;
        for (id, text) in Self::embedded() {
            let path = dir.join(format!("{id}.toml"));
            if path.exists() {
                continue;
            }
            std::fs::write(&path, text)?;
            n += 1;
        }
        Ok(n)
    }

    fn read_text(id: &str) -> Option<String> {
        for dir in Self::search_dirs() {
            let path = dir.join(format!("{id}.toml"));
            if let Ok(text) = std::fs::read_to_string(path) {
                return Some(text);
            }
        }
        Self::embedded()
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, text)| (*text).to_string())
    }

    fn search_dirs() -> Vec<PathBuf> {
        let mut dirs = vec![PathBuf::from("recipes")];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                dirs.push(parent.join("recipes"));
            }
        }
        dirs
    }
}
