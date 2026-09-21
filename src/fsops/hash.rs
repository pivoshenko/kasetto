//! Module that contains SHA-256 hashing of strings, files, and directory trees.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::error::{err, Result};

/// Mirrors `MAX_COPY_DEPTH` in `fsops::copy`: hashing follows symlinks the same
/// way copying does, so it needs the same cycle guard
const MAX_HASH_DEPTH: u32 = 32;

/// A file the digest covers: its logical path inside the tree, which supplies
/// the digest's name bytes, paired with the path its bytes are read from. The
/// two differ for a symlink, which `copy_dir` resolves and installs as a
/// regular file under the logical name.
type HashEntry = (PathBuf, PathBuf);

pub(crate) fn hash_dir(path: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(path, Path::new(""), &mut files, 0)?;
    files.sort();

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    for (logical, content) in files {
        // Normalize path separators so the digest is invariant across OSes
        // (Windows `\` vs Unix `/`); otherwise the same skill hashes differently
        // per platform and breaks committed-lock portability
        let rel = logical.to_string_lossy().replace('\\', "/");
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let file = fs::File::open(&content)?;
        let mut reader = BufReader::new(file);
        sha256_update_reader(&mut reader, &mut hasher, &mut buf)?;
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Hash an arbitrary string (used to key machine-local state by lock path).
pub(crate) fn hash_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hash a single file (for MCPs tracking).
pub(crate) fn hash_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buf = [0u8; 8192];
    sha256_update_reader(&mut reader, &mut hasher, &mut buf)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_update_reader<R: Read>(
    reader: &mut R,
    hasher: &mut Sha256,
    buf: &mut [u8; 8192],
) -> Result<()> {
    loop {
        let n = reader.read(buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(())
}

/// Walks `dir`, resolving symlinks exactly as `copy_dir` does. Hashing that
/// skipped symlinks while copying followed them left the lock's source digest
/// describing fewer files than every destination actually held, so the
/// destination could never match and each sync recopied the whole skill.
fn collect_files(dir: &Path, logical: &Path, out: &mut Vec<HashEntry>, depth: u32) -> Result<()> {
    if depth > MAX_HASH_DEPTH {
        return Err(err(format!(
            "hash depth limit ({MAX_HASH_DEPTH}) exceeded, possible symlink cycle at {}",
            dir.display()
        )));
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        let logical_path = logical.join(entry.file_name());
        if file_type.is_symlink() {
            let resolved = fs::canonicalize(&path)?;
            if fs::metadata(&resolved)?.is_dir() {
                collect_files(&resolved, &logical_path, out, depth + 1)?;
            } else {
                out.push((logical_path, resolved));
            }
        } else if file_type.is_dir() {
            collect_files(&path, &logical_path, out, depth + 1)?;
        } else if file_type.is_file() {
            out.push((logical_path, path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;

    /// The relative-path bytes fed into the digest must be separator-invariant:
    /// `a\b` and `a/b` must contribute identically so the same skill hashes the
    /// same on Windows and Unix.
    #[test]
    fn relative_path_separator_invariant() {
        let win = "a\\b\\c.md".replace('\\', "/");
        let unix = "a/b/c.md".replace('\\', "/");
        assert_eq!(win, unix);
    }

    #[test]
    fn hash_dir_is_stable_across_runs() {
        let root = temp_dir("kasetto-hash-stable");
        fs::create_dir_all(root.join("sub")).expect("create dirs");
        fs::write(root.join("SKILL.md"), "# Demo\n").expect("write");
        fs::write(root.join("sub/extra.md"), "body\n").expect("write");

        let a = hash_dir(&root).expect("hash a");
        let b = hash_dir(&root).expect("hash b");
        assert_eq!(a, b);

        let _ = fs::remove_dir_all(&root);
    }

    /// `copy_dir` resolves a file symlink and installs it as a regular file, so
    /// the digest has to cover that content too. Skipping it made the installed
    /// destination permanently unable to match the lock's source hash.
    #[cfg(unix)]
    #[test]
    fn hash_covers_symlinked_file_content() {
        let root = temp_dir("kasetto-hash-symlink");
        fs::create_dir_all(&root).expect("create dirs");
        fs::write(root.join("SKILL.md"), "# Demo\n").expect("write");
        fs::write(root.join("target.txt"), "payload\n").expect("write");
        std::os::unix::fs::symlink("target.txt", root.join("alias.txt")).expect("symlink");

        let with_link = hash_dir(&root).expect("hash with link");

        // The same tree with the symlink replaced by its resolved content must
        // hash identically: that is exactly what lands in the destination
        let plain = temp_dir("kasetto-hash-plain");
        fs::create_dir_all(&plain).expect("create dirs");
        fs::write(plain.join("SKILL.md"), "# Demo\n").expect("write");
        fs::write(plain.join("target.txt"), "payload\n").expect("write");
        fs::write(plain.join("alias.txt"), "payload\n").expect("write");

        assert_eq!(with_link, hash_dir(&plain).expect("hash plain"));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&plain);
    }

    /// Editing the target behind a symlink must move the digest, otherwise a
    /// changed skill still reports `unchanged`.
    #[cfg(unix)]
    #[test]
    fn hash_tracks_edits_behind_a_symlink() {
        let root = temp_dir("kasetto-hash-symlink-edit");
        fs::create_dir_all(&root).expect("create dirs");
        fs::write(root.join("target.txt"), "before\n").expect("write");
        std::os::unix::fs::symlink("target.txt", root.join("alias.txt")).expect("symlink");

        let before = hash_dir(&root).expect("hash before");
        fs::write(root.join("target.txt"), "after\n").expect("write");
        assert_ne!(before, hash_dir(&root).expect("hash after"));

        let _ = fs::remove_dir_all(&root);
    }

    /// Following symlinks introduces the cycle risk `copy_dir` already guards
    /// against, so the walk must terminate instead of recursing forever.
    #[cfg(unix)]
    #[test]
    fn hash_rejects_symlink_cycle() {
        let root = temp_dir("kasetto-hash-cycle");
        fs::create_dir_all(root.join("inner")).expect("create dirs");
        std::os::unix::fs::symlink(&root, root.join("inner/loop")).expect("symlink");

        let e = hash_dir(&root).expect_err("cycle must error");
        assert!(
            e.to_string().contains("depth limit"),
            "unexpected error: {e}"
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// A tree with no symlinks must keep the digest it had before hashing
    /// learned to follow them, or every committed lock would be invalidated.
    #[test]
    fn hash_unchanged_for_symlink_free_tree() {
        let root = temp_dir("kasetto-hash-nested");
        fs::create_dir_all(root.join("sub/deep")).expect("create dirs");
        fs::write(root.join("SKILL.md"), "# Demo\n").expect("write");
        fs::write(root.join("sub/extra.md"), "body\n").expect("write");
        fs::write(root.join("sub/deep/leaf.md"), "leaf\n").expect("write");

        assert_eq!(
            hash_dir(&root).expect("hash"),
            "d06ced109f573f70d5db8d5ce42010ae5978cf97fa2793aeae8d460f0343a8e1"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
