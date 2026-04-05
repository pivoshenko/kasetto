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
        let rel = f.strip_prefix(path)?.to_string_lossy();
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let file = fs::File::open(&f)?;
        let mut reader = BufReader::new(file);
        sha256_update_reader(&mut reader, &mut hasher, &mut buf)?;
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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
