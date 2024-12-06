use std::{
    fs,
    io::{self, Read},
    path::Path,
};
pub const KB: u64 = 1024;
pub const MB: u64 = 1024 * KB;
pub const GB: u64 = 1024 * MB;

pub fn read_exact_or_end<T: Read>(reader: &mut T, buffer: &mut [u8]) -> io::Result<usize> {
    let mut cur_idx = 0;
    loop {
        let subbuf = &mut buffer[cur_idx..];
        let read_count = match reader.read(subbuf) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        cur_idx += read_count;
        if read_count == 0 || cur_idx == buffer.len() {
            return Ok(cur_idx);
        }
    }
}

pub fn hard_link(left: &Path, right: &Path) -> io::Result<()> {
    let old_right_ext = right.extension().unwrap_or_default();
    let new_right_ext = {
        let mut buf = old_right_ext.to_os_string();
        buf.push(".bak");
        buf
    };
    let tmp_right_path = right.with_extension(new_right_ext);
    let mut did_backup = false;
    if !fs::exists(&tmp_right_path)? {
        fs::rename(right, &tmp_right_path)?;
        did_backup = true;
    }
    fs::hard_link(left, right)?;
    if did_backup {
        fs::remove_file(&tmp_right_path)?;
    }
    Ok(())
}
