use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy)]
pub struct ThumbConfig {
    pub thumbnail_size: u32,
    pub cache_lifetime_hrs: u64,
}

#[derive(Debug, Clone)]
pub enum ThumbStatus {
    Ready(PathBuf),
    Generating,
    Unavailable,
}

#[derive(Debug, Clone, Copy)]
pub struct ThumbStats {
    pub queued: u64,
    pub started: u64,
    pub completed: u64,
    pub failed: u64,
    pub dropped: u64,
}

const WORKER_COUNT: usize = 2;
const MAX_QUEUE_LEN: usize = 64;

#[derive(Debug)]
struct ThumbJob {
    source: PathBuf,
    output: PathBuf,
    key: String,
    thumbnail_size: u32,
}

#[derive(Default)]
struct QueueState {
    jobs: VecDeque<ThumbJob>,
    queued_keys: HashSet<String>,
    in_progress: HashSet<String>,
}

static CLEANUP_STARTED: OnceLock<()> = OnceLock::new();
static WORKERS_STARTED: OnceLock<()> = OnceLock::new();
static SHARED_STATE: OnceLock<Arc<(Mutex<QueueState>, Condvar)>> = OnceLock::new();
static STATS: ThumbStatsAtomic = ThumbStatsAtomic::new();

struct ThumbStatsAtomic {
    queued: AtomicU64,
    started: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
    dropped: AtomicU64,
}

impl ThumbStatsAtomic {
    const fn new() -> Self {
        Self {
            queued: AtomicU64::new(0),
            started: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        }
    }
}

pub fn start_cleanup_once(cfg: ThumbConfig) {
    let _ = CLEANUP_STARTED.get_or_init(|| {
        std::thread::spawn(move || {
            cleanup_expired(cfg.cache_lifetime_hrs);
        });
    });
}

pub fn stats_snapshot() -> ThumbStats {
    ThumbStats {
        queued: STATS.queued.load(Ordering::Relaxed),
        started: STATS.started.load(Ordering::Relaxed),
        completed: STATS.completed.load(Ordering::Relaxed),
        failed: STATS.failed.load(Ordering::Relaxed),
        dropped: STATS.dropped.load(Ordering::Relaxed),
    }
}

pub fn thumbnail_status(path: &Path, cfg: ThumbConfig) -> ThumbStatus {
    let Ok(abs) = absolute_path(path) else {
        return ThumbStatus::Unavailable;
    };
    let Ok(modified_epoch) = modified_epoch_secs(&abs) else {
        return ThumbStatus::Unavailable;
    };
    let key = cache_key(&abs, modified_epoch);

    let Ok(thumbs_dir) = ensure_thumbs_dir() else {
        return ThumbStatus::Unavailable;
    };
    let thumb_path = thumbs_dir.join(format!("{key}.png"));

    if thumb_path.exists() && is_fresh(&thumb_path, cfg.cache_lifetime_hrs) {
        return ThumbStatus::Ready(thumb_path);
    }

    queue_generation_if_needed(abs, thumb_path, key, cfg.thumbnail_size);
    ThumbStatus::Generating
}

fn queue_generation_if_needed(source: PathBuf, output: PathBuf, key: String, thumbnail_size: u32) {
    start_workers_once();
    let shared = SHARED_STATE
        .get_or_init(|| Arc::new((Mutex::new(QueueState::default()), Condvar::new())))
        .clone();
    let (lock, cv) = &*shared;
    let Ok(mut state) = lock.lock() else {
        return;
    };
    if state.queued_keys.contains(&key) || state.in_progress.contains(&key) {
        return;
    }

    if state.jobs.len() >= MAX_QUEUE_LEN {
        let _ = state.jobs.pop_front().map(|j| {
            state.queued_keys.remove(&j.key);
            STATS.dropped.fetch_add(1, Ordering::Relaxed);
        });
    }
    state.queued_keys.insert(key.clone());
    state.jobs.push_back(ThumbJob {
        source,
        output,
        key,
        thumbnail_size,
    });
    STATS.queued.fetch_add(1, Ordering::Relaxed);
    cv.notify_one();
}

fn start_workers_once() {
    let _ = WORKERS_STARTED.get_or_init(|| {
        let shared = SHARED_STATE
            .get_or_init(|| Arc::new((Mutex::new(QueueState::default()), Condvar::new())))
            .clone();
        for _ in 0..WORKER_COUNT {
            let state = shared.clone();
            std::thread::spawn(move || worker_loop(state));
        }
    });
}

fn worker_loop(shared: Arc<(Mutex<QueueState>, Condvar)>) {
    let (lock, cv) = &*shared;
    loop {
        let job = {
            let Ok(state) = lock.lock() else {
                return;
            };
            let mut state = cv
                .wait_while(state, |s| s.jobs.is_empty())
                .unwrap_or_else(|e| e.into_inner());
            let Some(job) = state.jobs.pop_front() else {
                continue;
            };
            state.queued_keys.remove(&job.key);
            state.in_progress.insert(job.key.clone());
            STATS.started.fetch_add(1, Ordering::Relaxed);
            job
        };

        let res = generate_thumbnail(&job.source, &job.output, job.thumbnail_size);
        if res.is_ok() {
            STATS.completed.fetch_add(1, Ordering::Relaxed);
        } else {
            STATS.failed.fetch_add(1, Ordering::Relaxed);
        }

        if let Ok(mut state) = lock.lock() {
            state.in_progress.remove(&job.key);
        }
    }
}

fn generate_thumbnail(source: &Path, output: &Path, thumbnail_size: u32) -> Result<(), ()> {
    let img = image::open(source).map_err(|_| ())?;
    let thumb = img.thumbnail(thumbnail_size, thumbnail_size);
    thumb
        .save_with_format(output, image::ImageFormat::Png)
        .map_err(|_| ())
}

fn cleanup_expired(cache_lifetime_hrs: u64) {
    let Ok(dir) = ensure_thumbs_dir() else {
        return;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        if !is_fresh(&path, cache_lifetime_hrs) {
            let _ = fs::remove_file(path);
        }
    }
}

fn is_fresh(path: &Path, cache_lifetime_hrs: u64) -> bool {
    let max_age = Duration::from_secs(cache_lifetime_hrs.saturating_mul(3600));
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    age <= max_age
}

fn ensure_thumbs_dir() -> Result<PathBuf, ()> {
    let mut dir = dirs::cache_dir().ok_or(())?;
    dir.push("dirt");
    dir.push("thumbs");
    fs::create_dir_all(&dir).map_err(|_| ())?;
    Ok(dir)
}

fn absolute_path(path: &Path) -> Result<PathBuf, ()> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map_err(|_| ())?
            .join(path)
            .canonicalize()
            .map_err(|_| ())
    }
}

fn modified_epoch_secs(path: &Path) -> Result<u64, ()> {
    let modified = fs::metadata(path).map_err(|_| ())?.modified().map_err(|_| ())?;
    modified
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| ())
}

fn cache_key(path: &Path, modified_epoch: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    format!("{:x}_{}", digest, modified_epoch)
}
