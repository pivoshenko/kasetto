use std::fs;
use std::path::Path;

use crate::error::{err, Result};

const MAX_COPY_DEPTH: u32 = 32;

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    copy_dir_contents(src, dst, 0)
}

fn copy_dir_contents(src: &Path, dst: &Path, depth: u32) -> Result<()> {
    if depth > MAX_COPY_DEPTH {
        return Err(err(format!(
            "copy depth limit ({MAX_COPY_DEPTH}) exceeded — possible symlink cycle at {}",
            src.display()
        )));
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let target = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            let resolved = fs::canonicalize(&src_path)?;
            let meta = fs::metadata(&resolved)?;
            if meta.is_dir() {
                fs::create_dir_all(&target)?;
                copy_dir_contents(&resolved, &target, depth + 1)?;
            } else {
                copy_file(&resolved, &target)?;
            }
        } else if file_type.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_contents(&src_path, &target, depth + 1)?;
        } else {
            copy_file(&src_path, &target)?;
        }
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    // fs::copy uses kernel-level copy where available and preserves
    // permissions, so executable scripts inside skills keep their +x bit.
    fs::copy(src, dst)?;
    // A propagated READONLY attribute would wedge every later re-sync on
    // Windows: remove_dir_all fails with PermissionDenied on read-only files.
    // Unix is unaffected (unlink is governed by the parent dir).
    #[cfg(windows)]
    {
        let mut perms = fs::metadata(dst)?.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            fs::set_permissions(dst, perms)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;

    #[cfg(unix)]
    #[test]
    fn copy_dir_preserves_executable_bit() {
        use std::os::unix::fs::PermissionsExt;

        let src = temp_dir("kasetto-copy-perm-src");
        fs::create_dir_all(&src).expect("create src");
        let script = src.join("run.sh");
        fs::write(&script, "#!/bin/sh\n").expect("write script");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");

        let dst = temp_dir("kasetto-copy-perm-dst");
        copy_dir(&src, &dst).expect("copy dir");

        let mode = fs::metadata(dst.join("run.sh"))
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "executable bit must survive the copy");

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_follows_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let src = temp_dir("kasetto-copy-src");
        let refs_dir = src.join("references");
        fs::create_dir_all(&refs_dir).expect("create refs");
        fs::write(refs_dir.join("guide.md"), "hello").expect("write file");
        symlink("references", src.join("linked-references")).expect("create symlink");

        let dst = temp_dir("kasetto-copy-dst");
        copy_dir(&src, &dst).expect("copy dir");

        assert!(dst.join("linked-references/guide.md").is_file());
        assert!(dst.join("references/guide.md").is_file());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }
}
