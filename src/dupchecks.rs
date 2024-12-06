use std::{
    fs::{self, File},
    io,
    os::unix::fs::MetadataExt,
    path::Path,
};

use log::{debug, trace};

use crate::{prompt_bool, read_exact_or_end, utils::MB, PromptUserMode};

/// The size of the buffer used when reading files for checking that they are
/// the same.
const COMPARE_READ_BUFFSIZE: usize = (32 * MB) as usize;

/// Check if 2 files are byte-for-byte identical.
pub fn is_same_file(left: &Path, right: &Path) -> Result<bool, io::Error> {
    debug!("Checking if paths {left:?} and {right:?} are the same file.");

    let left_meta = fs::symlink_metadata(left)?;
    let right_meta = fs::symlink_metadata(right)?;

    // 2 files of different types or sizes cannot be the same
    if left_meta.file_type() != right_meta.file_type() || left_meta.size() != right_meta.size() {
        return Ok(false);
    }
    trace!(
        "Files {} and {} passed size & type checks; size was {}.",
        left.display(),
        right.display(),
        left_meta.size()
    );

    // The same file is always identical to itself
    if left_meta.ino() == right_meta.ino() {
        return Ok(true);
    }
    trace!(
        "Files {} and {} pass ino short-circuit; were {} and {}.",
        left.display(),
        right.display(),
        left_meta.ino(),
        right_meta.ino()
    );

    debug!(
        "Files {} and {} passed simple metadata checks; now doing byte-by-byte comparison.",
        left.display(),
        right.display()
    );
    let mut left_fh = File::open(left)?;
    let mut left_buff = vec![0; COMPARE_READ_BUFFSIZE].into_boxed_slice();
    let mut right_fh = File::open(right)?;
    let mut right_buff = vec![0; COMPARE_READ_BUFFSIZE].into_boxed_slice();

    let mut idx = 0;

    loop {
        let read_left = read_exact_or_end(&mut left_fh, &mut left_buff)?;
        let left_subbuf = &left_buff[..read_left];
        let read_right = read_exact_or_end(&mut right_fh, &mut right_buff)?;
        let right_subbuf = &right_buff[..read_right];
        if left_subbuf != right_subbuf {
            debug!(
                "Found difference between {} and {} at offset {idx}.",
                left.display(),
                right.display()
            );
            return Ok(false);
        }

        // If the read byte count for the current iteration is smaller than the
        // buffer size, then we finished reading the file
        if read_left != left_buff.len() {
            debug!(
                "Finished comparison; files {} and {} are identical.",
                left.display(),
                right.display()
            );
            return Ok(true);
        }
        idx += read_left;
    }
}

/// The reason we shouldn't link 2 byte-for-byte identical files.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShouldNotRelinkReason {
    /// The files are located on different filesystems.
    DifferentFilesystems(u64, u64),
    /// The files are already hardlinked together.
    AlreadyLinked,
    /// The user told the application not to hardlink the files.
    UserSaidNo,
}

impl ShouldNotRelinkReason {
    /// Returns a end-user-displayable message for the given [ShouldNotRelinkReason].
    pub fn msg(&self) -> &'static str {
        match self {
            ShouldNotRelinkReason::AlreadyLinked => {
                "The files are already hard-linked to each other."
            }
            ShouldNotRelinkReason::DifferentFilesystems(_, _) => {
                "The files are on different filesystems."
            }
            ShouldNotRelinkReason::UserSaidNo => "The user said no.",
        }
    }
}

/// Checks if we should link a file, prompting the user if needed.
pub fn should_link(
    left: &Path,
    right: &Path,
    prompt_mode: PromptUserMode,
) -> Result<Result<(), ShouldNotRelinkReason>, io::Error> {
    let left_meta = std::fs::metadata(left)?;
    let right_meta = std::fs::metadata(right)?;

    if left_meta.ino() == right_meta.ino() {
        return Ok(Err(ShouldNotRelinkReason::AlreadyLinked));
    }

    if left_meta.dev() != right_meta.dev() {
        return Ok(Err(ShouldNotRelinkReason::DifferentFilesystems(
            left_meta.dev(),
            right_meta.dev(),
        )));
    }

    let user_resp = prompt_mode.as_default().unwrap_or_else(|| {
        let msg = format!(
            "Found candidates {} and {}. Should we hard-link them?",
            left.display(),
            right.display()
        );
        prompt_bool(&msg)
    });
    if user_resp {
        Ok(Ok(()))
    } else {
        Ok(Err(ShouldNotRelinkReason::UserSaidNo))
    }
}
