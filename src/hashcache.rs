use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    fs::File,
    hash::{Hash, Hasher},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::RwLock,
};

use log::trace;
use seahash::SeaHasher;

use crate::utils::{GB, MB};

const HASH_READ_BUFFSIZE: usize = 8 * 1024;

const MIN_SAMPLES: u32 = 2;
const MIN_SAMPLES_MAX: u64 = 1 * MB;
const MAX_SAMPLES: u32 = 4;
const MAX_SAMPLES_MIN: u64 = 16 * GB;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FileHashes {
    sea: u64,
    size: u64,
}

impl FileHashes {
    pub fn from_path(path: &Path) -> Result<Self, io::Error> {
        trace!("Now hashing {path:?}");

        let mut fh = File::open(path)?;
        let size = fh.seek(SeekFrom::End(0))?;
        let curpos = fh.seek(SeekFrom::Start(0))?;
        assert_ne!(0, size);
        assert_eq!(0, curpos);

        let skiplen = calculate_skiplen(size, HASH_READ_BUFFSIZE);

        let mut sea_hasher = SeaHasher::new();
        let mut buffer = vec![0; HASH_READ_BUFFSIZE].into_boxed_slice();
        let mut total_read = 0;
        let mut samples = 0;
        loop {
            let read_count = fh.read(&mut buffer)?;
            total_read += read_count;
            if read_count == 0 {
                break;
            }
            samples += 1;
            let subbuf = &buffer[..read_count];
            sea_hasher.write(subbuf);
            fh.seek(SeekFrom::Current(skiplen))?;
        }
        trace!("Finished hashing {path:?} using using {samples} samples ({total_read} bytes).");
        let sea = sea_hasher.finish();
        Ok(Self { sea, size })
    }
}

#[derive(Default)]
pub struct HashCache {
    inner: RwLock<HashMap<FileHashes, HashSet<PathBuf>>>,
}

impl HashCache {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&self, path: PathBuf, hashes: FileHashes) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(hashes)
            .or_default()
            .insert(path);
    }
    pub fn join(self, other: Self) -> Self {
        let mut inner = self.inner.into_inner().unwrap_or_else(|e| e.into_inner());
        for (k, v) in other.inner.into_inner().unwrap_or_else(|e| e.into_inner()) {
            inner.entry(k).or_default().extend(v);
        }
        Self {
            inner: RwLock::new(inner),
        }
    }

    pub fn duplicates(&self) -> Vec<HashSet<PathBuf>> {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
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

fn calculate_skiplen(filesize: u64, buffsize: usize) -> i64 {
    /*
    Thoughts:

    Lets scale it logarithmically.



    */
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
