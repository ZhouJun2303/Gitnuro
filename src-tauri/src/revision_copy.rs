use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

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
    ("cs", "script"),
    ("js", "script"),
    ("ts", "script"),
    ("tsx", "script"),
    ("jsx", "script"),
    ("rs", "script"),
    ("py", "script"),
    ("gd", "script"),
    ("lua", "script"),
    ("json", "data"),
    ("yaml", "data"),
    ("yml", "data"),
    ("xml", "data"),
    ("txt", "text"),
    ("md", "text"),
];

pub fn support(repo: &Path, file: &str) -> Option<Support> {
    let path = safe_path(file)?;
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if unity_asset(repo, &path) {
        if let Some((_, kind)) = UNITY.iter().find(|(ext, _)| *ext == extension) {
            return Some(Support {
                tool: "unity",
                kind,
            });
        }
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
            && repo.join(&prefix).join("ProjectSettings").is_dir()
            && repo.join(&prefix).join("Assets").is_dir()
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
        match OpenOptions::new().write(true).create_new(true).open(&dest) {
            Ok(mut output) => {
                if let Err(error) = output.write_all(&bytes) {
                    drop(output);
                    let _ = std::fs::remove_file(&dest);
                    return Err(error.to_string());
                }
                return Ok(dest.display().to_string());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Too many temporary copies of this revision".into())
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
        assert_eq!(support(&root, "Assets/item.unknown"), None);
        assert_eq!(support(&root, "Assets/../secrets.png"), None);
        assert_eq!(support(&root, ".git/config.txt"), None);
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
        let first = create(&root, &rev, "art/icon.png").unwrap();
        let second = create(&root, &rev, "art/icon.png").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(first).unwrap(), [0, 1, 2, 255]);
        assert_eq!(std::fs::read(second).unwrap(), [0, 1, 2, 255]);
        assert_eq!(std::fs::read(root.join("art/icon.png")).unwrap(), [9, 8, 7]);
        assert!(create(&root, &rev, "../art/icon.png").is_err());
        assert!(create(&root, "HEAD", "art/icon.png").is_err());
        std::fs::create_dir_all(root.join("Assets")).unwrap();
        std::fs::create_dir_all(root.join("ProjectSettings")).unwrap();
        std::fs::write(root.join("Assets/Player.cs"), b"class Player {}").unwrap();
        std::fs::write(root.join("Assets/Game.asmdef"), b"{}").unwrap();
        git(&["add", "Assets/Player.cs", "Assets/Game.asmdef"]);
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
        assert!(create(&root, &unity_rev, "Assets/Player.cs")
            .unwrap()
            .ends_with(".cs.txt"));
        assert!(create(&root, &unity_rev, "Assets/Game.asmdef")
            .unwrap()
            .ends_with(".asmdef.txt"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
