use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    fs::File,
    hash::{Hash, Hasher},
    io::{self, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use log::trace;
use seahash::SeaHasher;

use crate::{read_exact_or_end, utils::{GB, MB}};

/// The number of bytes in each sample.
const SAMPLE_SIZE: usize = 8 * 1024;

/// The minimum samples to take when hashing a file.
const MIN_SAMPLES: u32 = 2;
/// The maximum size of a file where we will take [MIN_SAMPLES] samples.
const MIN_SAMPLES_MAX: u64 = 1 * MB;
/// The maximum number to take when hashing a file (-1 due to modulo calculations).
const MAX_SAMPLES: u32 = 4;
/// The minimum size of a file where we will take [MAX_SAMPLES] samples.
const MAX_SAMPLES_MIN: u64 = 16 * GB;

/// A set of hash values to identify a file when looking for potential file
/// duplicates.
///
/// Note that it should NOT be assumed that 2 files with the same [FileHashes]
/// are identical; this structure explicitly and emphatically trades collision
/// detection accuracy for speed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FileHashes {
    sea: u64,
    size: u64,
}

impl FileHashes {
    /// Calculates the [FileHashes] for the file at the given path.
    pub fn from_path(path: &Path) -> Result<Self, io::Error> {
        trace!("Now hashing {path:?}");

        let mut fh = File::open(path)?;

        // Calculate the size using a seek-to-end to avoid the fs::metadata
        // call, which is very slow on certain platforms due to all the extra
        // information it pulls in
        let size = fh.seek(SeekFrom::End(0))?;

        let skiplen = calculate_skiplen(size, SAMPLE_SIZE);

        let mut sea_hasher = SeaHasher::new();
        let mut buffer = vec![0; SAMPLE_SIZE].into_boxed_slice();
        let mut total_read = 0;
        let mut samples = 0;
        loop {
            let read_count = read_exact_or_end(&mut fh, &mut buffer)?; 
            total_read += read_count;
            let subbuf = &buffer[..read_count];
            sea_hasher.write(subbuf);
            samples += 1;
            if read_count != buffer.len() {
                break;
            }
            fh.seek(SeekFrom::Current(skiplen))?;
        }
        trace!("Finished hashing {path:?} using using {samples} samples ({total_read} bytes).");
        let sea = sea_hasher.finish();
        Ok(Self { sea, size })
    }
}

/// A cache of files and their [FileHashes] for quick lookup of possible
/// duplicate candidates.
#[derive(Default)]
pub struct HashCache {
    inner: HashMap<FileHashes, HashSet<PathBuf>>,
}

impl HashCache {
    /// Construsts an empty [HashCache].
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a new path & associated [FileHashes] into this [HashCache].
    pub fn insert(&mut self, path: PathBuf, hashes: FileHashes) {
        self.inner.entry(hashes).or_default().insert(path);
    }

    /// Joins 2 [HashCache] collections into a single [HashCache].
    ///
    /// The returned values will have all hashes & files from both [self] and `other`.
    pub fn join(mut self, other: Self) -> Self {
        for (k, v) in other.inner {
            self.inner.entry(k).or_default().extend(v);
        }
        Self { inner: self.inner }
    }

    /// Retrieves the list of paths with duplicate hash values.
    ///
    /// Each entry of the returned list represents a set of paths with the same
    /// hash.
    pub fn duplicates(&self) -> Vec<HashSet<PathBuf>> {
        self.inner
            .values()
            .filter(|buf| buf.len() >= 2)
            .cloned()
            .collect()
    }
}

impl Debug for HashCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashCache")
            .field("inner", &"{..hashes..}")
            .finish()
    }
}

impl FromIterator<HashCache> for HashCache {
    fn from_iter<T: IntoIterator<Item = HashCache>>(iter: T) -> Self {
        let mut base = HashCache::new();
        for nxt in iter.into_iter() {
            base = base.join(nxt);
        }
        base
    }
}

/// Calculates the amount of the file to skip when reading blocks to calculate the hash.
///
// Why not just read the entire thing? Many files a user would want to run this
// program are are large; this program is a space-saving tool. As such, reading
// & calculating the hash for a 1+GB file is slow, spanning seconds, so if there
// are a large number of large files we're checking the hash calculation alone
// would take an absurd amount of time. Since the hash calculation's goal is
// already purely to speed up the program itself we sacrifise accuracy for speed
// and allow later steps to clean up our clumsiness.
fn calculate_skiplen(filesize: u64, buffsize: usize) -> i64 {
    let buffsize = buffsize as u64;
    if filesize <= (MIN_SAMPLES as u64) * buffsize {
        return 0;
    }

    let size_factor = filesize.ilog2();

    let samples = MIN_SAMPLES
        + (size_factor * (MAX_SAMPLES - MIN_SAMPLES))
            / (MAX_SAMPLES_MIN.ilog2() - MIN_SAMPLES_MAX.ilog2());
    let samples = samples.min(MAX_SAMPLES) as u64;
    ((filesize / samples) - buffsize) as i64
}
