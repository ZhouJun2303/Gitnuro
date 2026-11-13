//! A development window loads its frontend directly from the source checkout.
//! Deleting that checkout's files also tears down the running frontend and build.

use std::path::Path;

use awegit_git::Mutation;

pub(crate) fn check_delete(repo: &Path, request: &Mutation) -> Result<(), String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    check_source_delete(repo, source, cfg!(debug_assertions), request)
}

fn check_source_delete(
    repo: &Path,
    source: &Path,
    development: bool,
    request: &Mutation,
) -> Result<(), String> {
    if !development
        || !matches!(
            request,
            Mutation::Delete { .. } | Mutation::DeletePaths { .. }
        )
    {
        return Ok(());
    }
    // Canonical paths also catch aliases such as a Windows junction or `src/..`.
    let same_checkout = match (repo.canonicalize(), source.canonicalize()) {
        (Ok(repo), Ok(source)) => repo == source,
        _ => false,
    };
    if same_checkout {
        return Err("不能删除正在运行的开发版所用的源码工作副本文件。请使用已安装的 AweGit 操作此仓库，或在独立工作副本中测试。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Checkouts(PathBuf);

    impl Checkouts {
        fn new() -> Self {
            let id = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(".lab")
                .join("development-tests")
                .join(format!("{id}"));
            for checkout in ["source", "other"] {
                fs::create_dir_all(root.join(checkout).join("src")).unwrap();
            }
            Self(root)
        }
    }

    impl Drop for Checkouts {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    fn development_delete_reports_an_error_before_removing_files() {
        let checkouts = Checkouts::new();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        // Exercise the command boundary using only disposable files in `.lab`.
        let file = checkouts.0.join("source/src/lib.rs");
        fs::write(&file, "disposable source\n").unwrap();
        let relative = file
            .strip_prefix(source)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for request in [
            Mutation::Delete {
                file: relative.clone(),
            },
            Mutation::DeletePaths {
                files: vec![relative.clone()],
            },
        ] {
            let result = crate::mutate(Some(source.to_string_lossy().into_owned()), request, None);
            assert!(matches!(result, Err(error) if error.contains("已安装的 AweGit")));
            assert_eq!(fs::read_to_string(&file).unwrap(), "disposable source\n");
        }
    }

    #[test]
    fn development_rejects_single_and_batch_deletes_of_its_source_checkout() {
        let checkouts = Checkouts::new();
        let source = checkouts.0.join("source");
        for request in [
            Mutation::Delete {
                file: "src/lib.rs".into(),
            },
            Mutation::DeletePaths {
                files: vec!["src/lib.rs".into(), "src/en.ts".into()],
            },
        ] {
            let error = check_source_delete(&source, &source, true, &request).unwrap_err();
            assert!(error.contains("已安装的 AweGit"), "{error}");
            assert!(check_source_delete(&source.join("src/.."), &source, true, &request).is_err());
            assert!(
                check_source_delete(&source.canonicalize().unwrap(), &source, true, &request)
                    .is_err()
            );
        }
    }

    #[test]
    fn source_delete_guard_leaves_release_builds_and_other_checkouts_usable() {
        let checkouts = Checkouts::new();
        let source = checkouts.0.join("source");
        let other = checkouts.0.join("other");
        let request = Mutation::DeletePaths {
            files: vec!["src/lib.rs".into()],
        };
        assert!(check_source_delete(&source, &source, false, &request).is_ok());
        assert!(check_source_delete(&other, &source, true, &request).is_ok());
    }

    #[test]
    fn development_source_guard_allows_staging_and_recovering_deleted_files() {
        let checkouts = Checkouts::new();
        let source = checkouts.0.join("source");
        for request in [
            Mutation::StagePaths {
                files: vec!["src/lib.rs".into()],
                unstage: false,
            },
            Mutation::DiscardPaths {
                files: vec!["src/lib.rs".into()],
            },
            Mutation::RestoreFile {
                rev: "HEAD".into(),
                file: "src/lib.rs".into(),
            },
        ] {
            assert!(check_source_delete(&source, &source, true, &request).is_ok());
        }
    }
}
