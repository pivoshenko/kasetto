use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use crate::error::Result;

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    copy_dir_contents(src, dst)
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_contents(&src_path, &target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let reader = BufReader::new(fs::File::open(&src_path)?);
            let mut writer = BufWriter::new(fs::File::create(&target)?);
            let mut buf_reader = reader;
            std::io::copy(&mut buf_reader, &mut writer)?;
            writer.flush()?;
        }
    }
    Ok(())
}
