use std::{collections::HashSet, io::stdin, path::PathBuf, process::ExitCode};

use dupchecks::{is_same_file, should_link};
use hashcache::{FileHashes, HashCache};
use log::{debug, error, info, trace};
use utils::*;
use walkdir::WalkDir;
mod dupchecks;
mod hashcache;
mod utils;

fn init_logger() {
    let env = env_logger::Env::new()
        .filter_or("HLDUP_LOG", "TRACE")
        .write_style_or("HLDUP_COLOR", "auto");
    let mut logger = env_logger::Builder::from_env(env);
    logger.init();
}

fn main() -> ExitCode {
    init_logger();

    let args = match AppArgs::parse(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(v) => v,
        Err(msg) => {
            error!("{msg}");
            return ExitCode::FAILURE;
        }
    };
    trace!("Running with args: {args:?}");

    let cache = args
        .dirs
        .into_iter()
        .map(build_hash_cache)
        .collect::<HashCache>();
    dedup_files(&cache, args.prompt_mode);

    ExitCode::SUCCESS
}

#[derive(Debug)]
pub struct AppArgs {
    pub prompt_mode: PromptUserMode,
    pub dirs: Vec<PathBuf>,
}

impl AppArgs {
    pub fn parse(raw: &[impl AsRef<str>]) -> Result<Self, String> {
        let mut dirs = Vec::new();
        let mut prompt_mode = PromptUserMode::default();
        for arg in raw {
            let arg = arg.as_ref();
            match arg {
                "--prompt" => {
                    prompt_mode = PromptUserMode::Prompt;
                }
                "--default-yes" => {
                    prompt_mode = PromptUserMode::DefaultYes;
                }
                "--default-no" => {
                    prompt_mode = PromptUserMode::DefaultNo;
                }
                other => {
                    dirs.push(PathBuf::from(other));
                }
            }
        }
        if dirs.is_empty() {
            let curdir =
                std::env::current_dir().map_err(|e| format!("Error getting cwd: {e:?}"))?;
            dirs.push(curdir);
        }
        Ok(Self { dirs, prompt_mode })
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub enum PromptUserMode {
    DefaultYes,
    DefaultNo,
    #[default]
    Prompt,
}

impl PromptUserMode {
    pub const fn as_default(self) -> Option<bool> {
        match self {
            PromptUserMode::DefaultNo => Some(false),
            PromptUserMode::DefaultYes => Some(true),
            PromptUserMode::Prompt => None,
        }
    }
}

fn prompt_bool(msg: &str) -> bool {
    println!("{msg} [y/N]");
    let nextln = stdin().lines().next().unwrap().unwrap();
    const YES_RESPONSES: &[&str] = &["y", "Y", "yes", "Yes", "YES"];
    YES_RESPONSES.contains(&nextln.as_str())
}

pub fn build_hash_cache(root: PathBuf) -> HashCache {
    debug!("Building hashcache for root dir {root:?}");

    let mut retvl = HashCache::new();
    for ent in WalkDir::new(root) {
        let ent = match ent {
            Ok(v) => v,
            Err(e) => {
                error!("Found error walking directory tree: {e:?}");
                continue;
            }
        };
        if ent.file_type().is_dir() {
            trace!("Found directory {:?}; skipping.", ent.path());
            continue;
        }
        let path = if ent.path().is_absolute() {
            ent.path().to_owned()
        } else {
            match ent.path().canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    error!(
                        "Error finding absolute path for {}: {:?}.",
                        ent.path().display(),
                        e
                    );
                    continue;
                }
            }
        };
        debug!("Calculating hash for file {path:?}");
        let hash = match FileHashes::from_path(&path) {
            Ok(v) => v,
            Err(e) => {
                error!("Error getting file hash for {}: {:?}", path.display(), e);
                continue;
            }
        };
        retvl.insert(path, hash);
    }

    retvl
}

pub fn dedup_files(cache: &HashCache, prompt_mode: PromptUserMode) {
    let dups = cache.duplicates();
    info!("Found {} possible dupes.", dups.len());
    for flist in cache.duplicates() {
        if flist.len() <= 1 {
            continue;
        }
        let pairs = flist
            .iter()
            .flat_map(|left| flist.iter().map(move |right| (left, right)))
            .filter(|(left, right)| left != right)
            .map(|(left, right)| {
                if left < right {
                    (left, right)
                } else {
                    (right, left)
                }
            })
            .collect::<HashSet<_>>();
        for (left, right) in pairs {
            if left == right {
                continue;
            }
            match is_same_file(left, right) {
                Ok(false) => {
                    //TODO: Log
                    continue;
                }
                Ok(true) => {}
                Err(e) => {
                    error!(
                        "Error comparing files {} and {}: {:?}",
                        left.display(),
                        right.display(),
                        e
                    );
                    continue;
                }
            }
            info!(
                "Found candidates {} and {}.",
                left.display(),
                right.display()
            );
            match should_link(left, right, prompt_mode) {
                Err(e) => {
                    error!(
                        "IO Error checking candidacy of {} and {}: {:?}",
                        left.display(),
                        right.display(),
                        e
                    );
                    continue;
                }
                Ok(Err(reason)) => {
                    error!(
                        "Not linking {} and {}. Reason: {}",
                        left.display(),
                        right.display(),
                        reason.msg()
                    );
                    continue;
                }
                Ok(Ok(())) => {}
            }
            match hard_link(left, right) {
                Ok(()) => {
                    info!("Linked files {} and {}.", left.display(), right.display());
                }
                Err(e) => {
                    error!(
                        "Failed linking files {} and {}: {:?}.",
                        left.display(),
                        right.display(),
                        e
                    );
                }
            }
        }
    }
}
