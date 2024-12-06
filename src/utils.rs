use std::{
    fs,
    io::{self, Read},
    path::Path,
};


pub const KB: u64 = 1024;
pub const MB: u64 = 1024 * KB;
pub const GB: u64 = 1024 * MB;

/// Helper to pull bytes from a [Read]er into a buffer until either the buffer
/// is filled or we read the end of the [Read]er. Returns the number of bytes
/// read. 
/// 
/// If the `read_exact_or_end(rdr, buf)? != buf.len()` then it is guranteed that
/// `rdr` has reached `EOF`.
/// 
/// This is necessary since [Read::read] does not gurantee that the buffer being
/// filled means we've reached `EOF`, and [Read::read_exact] will return an
/// [io::Error] if it reaches `EOF` before filling the buffer. 
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

/// Wrapper around [std::fs::hard_link] that lets us overwrite existing files. 
/// 
/// # Implementation details
/// This enables overwriting by first checking if the previous file exists, and
/// if so renaming it and then deleting the renamed file once the
/// [std::fs::hard_link] call completes. 
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
