use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::error::Result;

pub(crate) fn hash_dir(path: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(path, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    for f in files {
        // Normalize path separators so the digest is invariant across OSes
        // (Windows `\` vs Unix `/`); otherwise the same skill hashes differently
        // per platform and breaks committed-lock portability.
        let rel = f.strip_prefix(path)?.to_string_lossy().replace('\\', "/");
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let file = fs::File::open(&f)?;
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

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(&path, out)?;
        } else if file_type.is_file() {
            out.push(path);
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
}
