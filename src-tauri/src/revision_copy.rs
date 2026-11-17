use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct CreatedCopy {
    repo: PathBuf,
    file: String,
    parent: PathBuf,
    digest: Vec<u8>,
    meta_existed: bool,
}

fn created_copies() -> &'static Mutex<HashMap<PathBuf, CreatedCopy>> {
    static COPIES: OnceLock<Mutex<HashMap<PathBuf, CreatedCopy>>> = OnceLock::new();
    COPIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn meta_path(copy: &Path) -> PathBuf {
    let mut name = copy.as_os_str().to_os_string();
    name.push(".meta");
    PathBuf::from(name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Support {
    tool: &'static str,
    kind: &'static str,
}

const UNITY: &[(&str, &str)] = &[
    ("prefab", "prefab"),
    ("unity", "scene"),
    ("asset", "asset"),
    ("mat", "material"),
    ("anim", "animation"),
    ("controller", "animation"),
    ("overridecontroller", "animation"),
    ("shader", "shader"),
    ("compute", "shader"),
    ("asmdef", "script"),
    ("uxml", "ui"),
    ("uss", "ui"),
];
const COMMON: &[(&str, &str)] = &[
    ("png", "image"),
    ("jpg", "image"),
    ("jpeg", "image"),
    ("gif", "image"),
    ("webp", "image"),
    ("tga", "image"),
    ("bmp", "image"),
    ("psd", "image"),
    ("svg", "image"),
    ("xls", "sheet"),
    ("xlsx", "sheet"),
    ("xlsm", "sheet"),
    ("xlsb", "sheet"),
    ("xlt", "sheet"),
    ("xltx", "sheet"),
    ("xltm", "sheet"),
    ("csv", "sheet"),
    ("tsv", "sheet"),
    ("ods", "sheet"),
    ("numbers", "sheet"),
    ("cs", "script"),
    ("java", "script"),
    ("kt", "script"),
    ("kts", "script"),
    ("js", "script"),
    ("ts", "script"),
    ("tsx", "script"),
    ("jsx", "script"),
    ("rs", "script"),
    ("py", "script"),
    ("gd", "script"),
    ("lua", "script"),
    ("json", "data"),
    ("gradle", "data"),
    ("properties", "data"),
    ("yaml", "data"),
    ("yml", "data"),
    ("xml", "data"),
    ("txt", "text"),
    ("md", "text"),
];

#[derive(Serialize)]
pub struct FormatCatalog {
    executables: Vec<&'static str>,
    groups: Vec<FormatGroup>,
}

#[derive(Serialize)]
pub struct FormatGroup {
    tool: &'static str,
    scope: &'static str,
    kinds: Vec<FormatKind>,
}

#[derive(Serialize)]
pub struct FormatKind {
    kind: &'static str,
    extensions: Vec<String>,
}

fn grouped_formats(formats: impl Iterator<Item = (&'static str, &'static str)>) -> Vec<FormatKind> {
    let mut kinds: Vec<FormatKind> = Vec::new();
    for (ext, kind) in formats {
        if let Some(entry) = kinds.iter_mut().find(|entry| entry.kind == kind) {
            entry.extensions.push(format!(".{ext}"));
        } else {
            kinds.push(FormatKind {
                kind,
                extensions: vec![format!(".{ext}")],
            });
        }
    }
    kinds
}

pub fn formats() -> FormatCatalog {
    FormatCatalog {
        executables: vec![],
        groups: vec![
            FormatGroup {
                tool: "unity",
                scope: "any",
                kinds: grouped_formats(UNITY.iter().copied()),
            },
            FormatGroup {
                tool: "unity",
                scope: "unityAssets",
                kinds: grouped_formats(
                    COMMON
                        .iter()
                        .copied()
                        .filter(|(ext, kind)| *kind == "image" || *ext == "cs"),
                ),
            },
            FormatGroup {
                tool: "common",
                scope: "any",
                kinds: grouped_formats(COMMON.iter().copied()),
            },
        ],
    }
}

pub fn support(repo: &Path, file: &str) -> Option<Support> {
    let path = safe_path(file)?;
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if let Some((_, kind)) = UNITY.iter().find(|(ext, _)| *ext == extension) {
        return Some(Support {
            tool: "unity",
            kind,
        });
    }
    if unity_asset(repo, &path) {
        if let Some((_, kind)) = COMMON
            .iter()
            .find(|(ext, kind)| *ext == extension && (*kind == "image" || *ext == "cs"))
        {
            return Some(Support {
                tool: "unity",
                kind,
            });
        }
    }
    COMMON
        .iter()
        .find(|(ext, _)| *ext == extension)
        .map(|(_, kind)| Support {
            tool: "common",
            kind,
        })
}

fn unity_asset(repo: &Path, path: &Path) -> bool {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        if component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("Assets")
            && repo.join(&prefix).join("Assets").is_dir()
            && (prefix.as_os_str().is_empty()
                || repo.join(&prefix).join("ProjectSettings").is_dir())
        {
            return true;
        }
        prefix.push(component);
    }
    false
}

fn safe_path(file: &str) -> Option<PathBuf> {
    if file.is_empty() || file.contains([':', '\0', '\n', '\r']) {
        return None;
    }
    let mut path = PathBuf::new();
    for part in file.split(['/', '\\']) {
        if part.is_empty() || part == "." || part == ".." || part.eq_ignore_ascii_case(".git") {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

pub fn create(repo: &Path, rev: &str, file: &str) -> Result<String, String> {
    let kind = support(repo, file).ok_or("Unsupported file type or path")?;
    if !(rev.len() == 40 || rev.len() == 64) || !rev.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid commit ID".into());
    }
    let relative = safe_path(file).ok_or("Invalid file path")?;
    let parent = repo
        .join(&relative)
        .parent()
        .ok_or("Invalid file path")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let root = repo.canonicalize().map_err(|e| e.to_string())?;
    if !parent.starts_with(&root) {
        return Err("File is outside the repository".into());
    }
    let stem = relative
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid file name")?;
    let ext = relative
        .extension()
        .and_then(|s| s.to_str())
        .ok_or("Invalid file extension")?;
    let suffix = if kind.tool == "unity"
        && (ext.eq_ignore_ascii_case("cs") || ext.eq_ignore_ascii_case("asmdef"))
    {
        format!("{ext}.txt")
    } else {
        ext.to_string()
    };
    let bytes = awegit_git::revision_bytes(repo, rev, file).map_err(|e| e.to_string())?;
    for index in 0..1000 {
        let number = if index == 0 {
            String::new()
        } else {
            format!("-{index}")
        };
        let dest = parent.join(format!("{stem}.awegit-{}{}.{}", &rev[..12], number, suffix));
        let meta_existed = std::fs::symlink_metadata(meta_path(&dest)).is_ok();
        match OpenOptions::new().write(true).create_new(true).open(&dest) {
            Ok(mut output) => {
                if let Err(error) = output.write_all(&bytes) {
                    drop(output);
                    let _ = std::fs::remove_file(&dest);
                    return Err(error.to_string());
                }
                created_copies()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(
                        dest.clone(),
                        CreatedCopy {
                            repo: root.clone(),
                            file: file.to_string(),
                            parent: parent.clone(),
                            digest: Sha256::digest(&bytes).to_vec(),
                            meta_existed,
                        },
                    );
                return Ok(dest.display().to_string());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Too many temporary copies of this revision".into())
}

pub fn remove(repo: &Path, file: &str, copy: &str, force: bool) -> Result<(), String> {
    let dest = PathBuf::from(copy);
    let mut registry = created_copies().lock().unwrap_or_else(|e| e.into_inner());
    let record = registry
        .get(&dest)
        .ok_or("This preview was not created in this session")?;
    let root = repo.canonicalize().map_err(|e| e.to_string())?;
    if root != record.repo
        || file != record.file
        || dest.parent().and_then(|p| p.canonicalize().ok()) != Some(record.parent.clone())
    {
        return Err("Preview path or repository changed".into());
    }

    match std::fs::symlink_metadata(&dest) {
        Ok(metadata) if metadata.file_type().is_file() => {
            if !force {
                let bytes = std::fs::read(&dest).map_err(|e| e.to_string())?;
                if Sha256::digest(&bytes).to_vec() != record.digest {
                    return Err("Preview was modified; it was not deleted".into());
                }
            }
            std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
        }
        Ok(_) => return Err("Preview is no longer a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }

    if !record.meta_existed {
        let meta = meta_path(&dest);
        match std::fs::symlink_metadata(&meta) {
            Ok(metadata) if metadata.file_type().is_file() => {
                std::fs::remove_file(meta).map_err(|e| e.to_string())?
            }
            Ok(_) => return Err("Preview metadata is no longer a regular file".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    registry.remove(&dest);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_unity_and_common_formats() {
        let root = std::env::temp_dir().join(format!("awegit-copy-kind-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Assets")).unwrap();
        std::fs::create_dir_all(root.join("ProjectSettings")).unwrap();
        assert_eq!(
            support(&root, "Assets/Props/item.prefab"),
            Some(Support {
                tool: "unity",
                kind: "prefab"
            })
        );
        std::fs::create_dir_all(root.join("nested/ProjectSettings")).unwrap();
        std::fs::create_dir_all(root.join("nested/Assets")).unwrap();
        assert_eq!(
            support(&root, "nested/Assets/item.prefab"),
            Some(Support {
                tool: "unity",
                kind: "prefab"
            })
        );
        assert_eq!(
            support(&root, "Assets/Props/icon.PNG"),
            Some(Support {
                tool: "unity",
                kind: "image"
            })
        );
        assert_eq!(
            support(&root, "docs/icon.png"),
            Some(Support {
                tool: "common",
                kind: "image"
            })
        );
        assert_eq!(
            support(&root, "Assets/item.gd"),
            Some(Support {
                tool: "common",
                kind: "script"
            })
        );
        assert_eq!(
            support(
                &root,
                "unityLibrary/src/main/java/com/unity3d/player/GoogleHelper.java"
            ),
            Some(Support {
                tool: "common",
                kind: "script"
            })
        );
        assert_eq!(
            support(&root, "unityLibrary/build.gradle"),
            Some(Support {
                tool: "common",
                kind: "data"
            })
        );
        assert_eq!(support(&root, "Assets/item.unknown"), None);
        for ext in [
            "xls", "xlsx", "xlsm", "xlsb", "xlt", "xltx", "xltm", "csv", "tsv", "ods", "numbers",
        ] {
            for folder in ["", "data/", "Assets/", "nested/Assets/", "表格\\"] {
                assert_eq!(
                    support(&root, &format!("{folder}预算表.{}", ext.to_ascii_uppercase())),
                    Some(Support {
                        tool: "common",
                        kind: "sheet"
                    })
                );
            }
        }
        assert_eq!(support(&root, "Assets/../secrets.png"), None);
        assert_eq!(support(&root, ".git/config.txt"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn format_catalog_tracks_supported_extensions() {
        let catalog = formats();
        assert!(catalog.executables.is_empty());
        assert_eq!(catalog.groups[0].tool, "unity");
        assert_eq!(catalog.groups[1].scope, "unityAssets");
        assert!(catalog.groups[0]
            .kinds
            .iter()
            .any(|entry| entry.extensions.contains(&".prefab".to_string())));
        assert!(catalog.groups[1]
            .kinds
            .iter()
            .any(|entry| entry.extensions.contains(&".png".to_string())));
        assert!(catalog.groups[2]
            .kinds
            .iter()
            .any(|entry| entry.extensions.contains(&".md".to_string())));
        for (ext, _) in UNITY.iter().chain(COMMON.iter()) {
            assert!(catalog
                .groups
                .iter()
                .flat_map(|group| &group.kinds)
                .any(|kind| kind.extensions.contains(&format!(".{ext}"))));
        }
    }

    #[test]
    fn unity_assets_work_without_project_settings() {
        let root = std::env::temp_dir().join(format!("awegit-assets-only-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Assets/ResBundles/Prefabs/UI")).unwrap();
        assert_eq!(
            support(&root, "Assets/ResBundles/Prefabs/UI/UIChapter.prefab"),
            Some(Support {
                tool: "unity",
                kind: "prefab"
            })
        );
        assert_eq!(
            support(&root, "Assets/Script/Player.cs"),
            Some(Support {
                tool: "unity",
                kind: "script"
            })
        );
        assert_eq!(
            support(&root, "Assets/UI/icon.png"),
            Some(Support {
                tool: "unity",
                kind: "image"
            })
        );
        assert_eq!(
            support(&root, "Other/UIChapter.prefab"),
            Some(Support {
                tool: "unity",
                kind: "prefab"
            })
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn writes_historical_bytes_without_replacing_existing_copies() {
        use std::process::Command;
        let root = std::env::temp_dir().join(format!(
            "awegit-copy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("art")).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_string()
        };
        git(&["init", "-q", "--template="]);
        std::fs::write(root.join("art/icon.png"), [0, 1, 2, 255]).unwrap();
        git(&["add", "art/icon.png"]);
        git(&[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "test: image",
        ]);
        let rev = git(&["rev-parse", "HEAD"]);
        std::fs::write(root.join("art/icon.png"), [9, 8, 7]).unwrap();
        let preserved_meta = root.join(format!("art/icon.awegit-{}.png.meta", &rev[..12]));
        std::fs::write(&preserved_meta, b"existing metadata").unwrap();
        let first = create(&root, &rev, "art/icon.png").unwrap();
        let second = create(&root, &rev, "art/icon.png").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), [0, 1, 2, 255]);
        assert_eq!(std::fs::read(&second).unwrap(), [0, 1, 2, 255]);
        assert_eq!(std::fs::read(root.join("art/icon.png")).unwrap(), [9, 8, 7]);
        assert!(create(&root, &rev, "../art/icon.png").is_err());
        assert!(create(&root, "HEAD", "art/icon.png").is_err());
        assert!(remove(
            &root,
            "art/icon.png",
            root.join("art/icon.png").to_str().unwrap(),
            false
        )
        .is_err());
        assert!(remove(&root, "other.png", &first, false).is_err());
        remove(&root, "art/icon.png", &first, false).unwrap();
        assert!(!Path::new(&first).exists());
        assert_eq!(
            std::fs::read(&preserved_meta).unwrap(),
            b"existing metadata"
        );
        let generated_meta = meta_path(Path::new(&second));
        std::fs::write(&generated_meta, b"generated metadata").unwrap();
        remove(&root, "art/icon.png", &second, false).unwrap();
        assert!(!Path::new(&second).exists());
        assert!(!generated_meta.exists());
        assert_eq!(std::fs::read(root.join("art/icon.png")).unwrap(), [9, 8, 7]);
        std::fs::create_dir_all(root.join("Assets")).unwrap();
        std::fs::create_dir_all(root.join("ProjectSettings")).unwrap();
        std::fs::write(root.join("Assets/Player.cs"), b"class Player {}").unwrap();
        std::fs::write(root.join("Assets/Game.asmdef"), b"{}").unwrap();
        std::fs::write(root.join("Assets/UIChapter.prefab"), b"%YAML 1.1\n").unwrap();
        git(&[
            "add",
            "Assets/Player.cs",
            "Assets/Game.asmdef",
            "Assets/UIChapter.prefab",
        ]);
        git(&[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "test: unity assets",
        ]);
        let unity_rev = git(&["rev-parse", "HEAD"]);
        std::fs::remove_dir(root.join("ProjectSettings")).unwrap();
        assert!(create(&root, &unity_rev, "Assets/Player.cs")
            .unwrap()
            .ends_with(".cs.txt"));
        assert!(create(&root, &unity_rev, "Assets/Game.asmdef")
            .unwrap()
            .ends_with(".asmdef.txt"));
        let prefab = create(&root, &unity_rev, "Assets/UIChapter.prefab").unwrap();
        assert!(prefab.ends_with(".prefab"));
        assert_eq!(std::fs::read(&prefab).unwrap(), b"%YAML 1.1\n");
        std::fs::write(&prefab, b"edited in Unity").unwrap();
        assert!(remove(&root, "Assets/UIChapter.prefab", &prefab, false).is_err());
        assert_eq!(std::fs::read(&prefab).unwrap(), b"edited in Unity");
        remove(&root, "Assets/UIChapter.prefab", &prefab, true).unwrap();
        assert!(!Path::new(&prefab).exists());
        assert_eq!(
            std::fs::read(root.join("Assets/UIChapter.prefab")).unwrap(),
            b"%YAML 1.1\n"
        );
        std::fs::create_dir_all(root.join("unityLibrary/src/main/java/com/unity3d/player"))
            .unwrap();
        std::fs::write(
            root.join("unityLibrary/src/main/java/com/unity3d/player/GoogleHelper.java"),
            b"class GoogleHelper {}",
        )
        .unwrap();
        git(&[
            "add",
            "unityLibrary/src/main/java/com/unity3d/player/GoogleHelper.java",
        ]);
        git(&[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "test: android source",
        ]);
        let android_rev = git(&["rev-parse", "HEAD"]);
        let java = create(
            &root,
            &android_rev,
            "unityLibrary/src/main/java/com/unity3d/player/GoogleHelper.java",
        )
        .unwrap();
        assert!(java.ends_with(".java"));
        assert_eq!(std::fs::read(java).unwrap(), b"class GoogleHelper {}");
        std::fs::remove_dir_all(root).unwrap();
    }
}
