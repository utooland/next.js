#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
mod batch_schedule;
mod fs_api;
#[cfg(test)]
mod mock_fs_api;

use std::{
    any::Any,
    borrow::Cow,
    collections::BTreeSet,
    fmt,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use std::{
    env,
    sync::{
        LazyLock,
        mpsc::{Receiver, RecvTimeoutError, channel},
    },
};

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use anyhow::Context;
use anyhow::Result;
use bincode::{
    Decode, Encode,
    de::Decoder,
    enc::Encoder,
    error::{DecodeError, EncodeError},
};
use bitflags::bitflags;
use indexmap::map::{RawEntryApiV1, raw_entry_v1::RawEntryMut};
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use notify::{Config, PollWatcher, RecommendedWatcher, Watcher};
use notify_types::event::{Event, EventKind, MetadataKind, ModifyKind, RenameMode};
use regex::RegexSet;
use rustc_hash::FxHashSet;
use tokio::sync::{RwLock, RwLockWriteGuard};
use tracing::instrument;
use turbo_rcstr::RcStr;
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use turbo_tasks::spawn_thread;
use turbo_tasks::{
    FxIndexMap, FxIndexSet, InvalidationReason, InvalidationReasonKind, Invalidator, ResolvedVc,
    TraitRef, TurboTasksApi, trace::TraceRawVcs, util::StaticOrArc,
};

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use crate::invalidation::WatchStart;
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use crate::watcher::batch_schedule::BatchSchedule;
use crate::{
    format_absolute_fs_path, invalidation::WatchChange, invalidator_map::InvalidatorMap,
    path_map::OrderedPathMapExt, watcher::fs_api::DiskFileSystemWatcherApi,
};

/// Overrides [`DiskWatcherConfig::recursive_mode`]. Users shouldn't need to set this, this is
/// intended only for debugging purposes.
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
static FORCED_WATCH_RECURSIVE_MODE: LazyLock<Option<DiskWatcherRecursiveMode>> = LazyLock::new(
    || match env::var("TURBO_TASKS_FORCE_WATCH_MODE").as_deref() {
        Ok("recursive") => Some(DiskWatcherRecursiveMode::Recursive),
        Ok("nonrecursive") => Some(DiskWatcherRecursiveMode::NonRecursive),
        Ok(_) => {
            eprintln!(
                "unsupported `TURBO_TASKS_FORCE_WATCH_MODE`, must be `recursive` or `nonrecursive`"
            );
            None
        }
        _ => None,
    },
);

#[turbo_tasks::task_input]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, TraceRawVcs, Encode, Decode)]
pub struct DiskWatcherConfig {
    /// Whether to let the [`notify::Watcher`] recurse into subdirectories itself, or to track and
    /// watch each directory we care about ourselves.
    ///
    /// [`None`] picks a default based on the platform and [`Self::poll_interval`], which is
    /// normally what you want.
    ///
    /// The `TURBO_TASKS_FORCE_WATCH_MODE` environment variable will override this configuration
    /// value (intended for debugging purposes).
    pub recursive_mode: Option<DiskWatcherRecursiveMode>,
    /// Poll the filesystem at this interval instead of using the platform's native file watching,
    /// for cases where native watching doesn't work (e.g. some Docker setups). This is slow and
    /// inefficient, it should only be used as a last resort.
    ///
    /// [`None`] disables polling, using the platform's native watcher ([`RecommendedWatcher`])
    /// instead of [`PollWatcher`].
    pub poll_interval: Option<Duration>,
    /// Attach an [`InvalidationReason`] to every invalidation the watcher causes ([`WatchStart`],
    /// [`WatchChange`], or [`InvalidateRescan`]), so that it can be reported to the user.
    ///
    /// This costs an extra allocation per invalidated path, so it's only worth enabling when
    /// something actually consumes the reasons.
    pub report_invalidation_reason: bool,

    /// How long to keep a batch of filesystem events open, waiting for more events, before
    /// flushing invalidations. Batching coalesces bursts (e.g. a `git checkout`) into a single
    /// invalidation pass and avoids reading half-written files.
    ///
    /// If set too low (<10ms), this is known to cause partial file reads on Linux where `inotify`
    /// has very low latency.
    pub batch_delay: Duration,
    /// When [`DiskWatcherPathMatcher::match_path`] returns `true`, we will extend the batch by
    /// [`Self::extended_batch_delay_duration`].
    pub extended_batch_delay_matcher: Option<ResolvedVc<Box<dyn DiskWatcherPathMatcher>>>,
    /// The idle period required to close a batch once [`Self::extended_batch_delay_matcher`] has
    /// matched. Unused when there is no matcher.
    pub extended_batch_delay_duration: Duration,

    /// If a single batch stays open at least this long, emit a `FilesystemSettlingEvent`
    /// compilation event so the user knows why work has stalled. Repeated events within the same
    /// batch back off exponentially, up to [`Self::settling_event_max_delay`].
    pub settling_event_initial_delay: Duration,
    /// Upper bound for the exponentially increasing interval between repeated
    /// `FilesystemSettlingEvent`s within a single batch.
    pub settling_event_max_delay: Duration,
}

impl Default for DiskWatcherConfig {
    fn default() -> Self {
        Self {
            recursive_mode: None,
            poll_interval: None,
            report_invalidation_reason: false,
            batch_delay: Duration::from_millis(10),
            extended_batch_delay_matcher: None,
            extended_batch_delay_duration: Duration::from_millis(200),
            settling_event_initial_delay: Duration::from_millis(500),
            settling_event_max_delay: Duration::from_secs(60),
        }
    }
}

/// Matches absolute paths reported by the filesystem watcher. See
/// [`DiskWatcherConfig::extended_batch_delay_matcher`].
#[turbo_tasks::value_trait]
pub trait DiskWatcherPathMatcher {
    /// Called on the watcher thread once per path of every incoming event, so this should be
    /// cheap and must not block.
    fn match_path(&self, path: &Path) -> bool;
}

/// Equivalent to [`notify::RecursiveMode`], but implements traits needed by [`turbo_tasks`].
///
/// When using [`Self::Recursive`], [`notify::Watcher`] will recursively track all contents
/// of the filesystem root. This should only be used on platforms with efficient recursive watcher
/// implementations (i.e. macOS and Windows).
///
/// When using [`Self::NonRecursive`], we only track previously read files and their parent
/// directories.
#[turbo_tasks::task_input]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, TraceRawVcs, Encode, Decode)]
pub enum DiskWatcherRecursiveMode {
    Recursive,
    NonRecursive,
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
impl From<DiskWatcherRecursiveMode> for notify::RecursiveMode {
    fn from(value: DiskWatcherRecursiveMode) -> Self {
        match value {
            DiskWatcherRecursiveMode::Recursive => notify::RecursiveMode::Recursive,
            DiskWatcherRecursiveMode::NonRecursive => notify::RecursiveMode::NonRecursive,
        }
    }
}

impl DiskWatcherConfig {
    /// Resolves [`Self::recursive_mode`], falling back to a default based on
    /// [`Self::poll_interval`] and the platform.
    fn resolve_recursive_mode(&self) -> DiskWatcherRecursiveMode {
        // macOS and Windows have efficient recursive watchers, so it's best to track the entire
        // directory and filter events to the files we care about. inotify on Linux is
        // non-recursive, so notify-rs's implementation is inefficient; better for us to track it
        // ourselves and only watch the directories we know we care about.
        //
        // See: <https://github.com/vercel/turborepo/pull/4100>
        let platform_has_efficient_recursive_watcher = cfg!(any(
            target_os = "macos",
            target_os = "windows",
            all(target_family = "wasm", target_os = "unknown")
        ));

        // the env var is a debugging escape hatch, so it wins over everything else
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        if let Some(forced) = *FORCED_WATCH_RECURSIVE_MODE {
            return forced;
        }

        if let Some(recursive_mode) = self.recursive_mode {
            recursive_mode
        } else if self.poll_interval.is_some() {
            // `PollWatcher` implements recursive watching by walking the entire subtree on every
            // poll, so watching the fs root recursively would stat every file in the project each
            // interval. Watching non-recursively keeps each poll to the directories we've read.
            DiskWatcherRecursiveMode::NonRecursive
        } else if platform_has_efficient_recursive_watcher {
            DiskWatcherRecursiveMode::Recursive
        } else {
            DiskWatcherRecursiveMode::NonRecursive
        }
    }
}

pub(crate) struct DiskWatcher {
    state: State,
    config: DiskWatcherConfig,
    /// Original ignore configuration, retained so decoded watchers preserve package opt-ins.
    ignored_path_config: Arc<Vec<RcStr>>,
    /// Paths to ignore when watching for file changes
    ignored_paths: Arc<Vec<RcStr>>,
    /// Package name regular expressions that opt dependencies back into watching.
    node_modules_regexes: Option<Arc<RegexSet>>,
}

/// A decoded [`DiskWatcher`] is always stopped. The compiled regex set is reconstructed from the
/// serialized ignored-path configuration.
impl Encode for DiskWatcher {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.config.encode(encoder)?;
        self.ignored_path_config.as_ref().encode(encoder)
    }
}

impl<Ctx> Decode<Ctx> for DiskWatcher {
    fn decode<D: Decoder<Context = Ctx>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let config = DiskWatcherConfig::decode(decoder)?;
        let ignored_paths = Vec::<RcStr>::decode(decoder)?;
        Ok(Self::new_with_ignored_paths(config, ignored_paths))
    }
}
bincode::impl_borrow_decode!(DiskWatcher);

enum State {
    // Note: Information about if we're a recursive or non-recursive watcher must live outside the
    // `RwLock` to allow us to quickly bail out before calling functions in
    // `non_recursive_helpers`.
    Recursive(RwLock<RecursiveState>),
    NonRecursive(RwLock<NonRecursiveState>),
}

enum StateWriteGuard<'a> {
    Recursive(RwLockWriteGuard<'a, RecursiveState>),
    NonRecursive(RwLockWriteGuard<'a, NonRecursiveState>),
}

impl State {
    fn new_stopped(recursive_mode: DiskWatcherRecursiveMode) -> Self {
        match recursive_mode {
            DiskWatcherRecursiveMode::Recursive => {
                Self::Recursive(RwLock::new(RecursiveState::Stopped))
            }
            DiskWatcherRecursiveMode::NonRecursive => {
                Self::NonRecursive(RwLock::new(NonRecursiveState::Stopped))
            }
        }
    }

    async fn write(&self) -> StateWriteGuard<'_> {
        match self {
            Self::Recursive(state) => StateWriteGuard::Recursive(state.write().await),
            Self::NonRecursive(state) => StateWriteGuard::NonRecursive(state.write().await),
        }
    }

    fn recursive_mode(&self) -> DiskWatcherRecursiveMode {
        match self {
            Self::Recursive(_) => DiskWatcherRecursiveMode::Recursive,
            Self::NonRecursive(_) => DiskWatcherRecursiveMode::NonRecursive,
        }
    }
}

/// Used when [`DiskWatcherConfig::recursive_mode`] returns [`RecursiveMode::Recursive`] (default on
/// macOS and Windows when not polling).
enum RecursiveState {
    /// Used when [`DiskWatcher::start_watching`] hasn't been called yet or after
    /// [`DiskWatcher::stop_watching`] is called.
    Stopped,
    Watching {
        /// Hold onto the watcher: When this is dropped, it will cause the channel to disconnect
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        _notify_watcher: NotifyWatcher,
    },
}

/// Used when [`DiskWatcherConfig::recursive_mode`] returns [`RecursiveMode::NonRecursive`] (default
/// on Linux, and everywhere when polling).
enum NonRecursiveState {
    /// Used when [`DiskWatcher::start_watching`] hasn't been called yet or after
    /// [`DiskWatcher::stop_watching`] is called.
    Stopped,
    Watching(NonRecursiveWatchingState),
}

// split out from the `NonRecursiveState` enum because we want to pass this value around
struct NonRecursiveWatchingState {
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    notify_watcher: NotifyWatcher,
    /// Keeps track of which directories are currently or were previously watched by
    /// [`Self::notify_watcher`].
    ///
    /// Invariants:
    /// - Never contains `root_path`. A watcher for `root_path` is implicitly set up during
    ///   [`DiskWatcher::start_watching`].
    /// - Contains all parent directories up to `root_path` for every entry.
    watched: BTreeSet<PathBuf>,
}

/// A thin wrapper around [`RecommendedWatcher`] and [`PollWatcher`].
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
enum NotifyWatcher {
    Recommended(RecommendedWatcher),
    Polling(PollWatcher),
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
impl NotifyWatcher {
    fn watch(&mut self, path: &Path, recursive_mode: notify::RecursiveMode) -> notify::Result<()> {
        match self {
            Self::Recommended(watcher) => watcher.watch(path, recursive_mode),
            Self::Polling(watcher) => watcher.watch(path, recursive_mode),
        }
    }
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
mod non_recursive_helpers {
    use super::*;
    use crate::path_map::OrderedPathSetExt;

    /// Called after a rescan in case a previously watched-but-deleted directory was recreated.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[instrument(skip_all, level = "trace")]
    pub async fn restore_all_watched_ignore_errors(
        state: &RwLock<NonRecursiveState>,
        root_path: &Path,
    ) {
        let mut guard = state.write().await;
        let NonRecursiveState::Watching(watching_state) = &mut *guard else {
            return;
        };
        for dir_path in watching_state.watched.iter() {
            // TODO: Report diagnostics if this error happens
            //
            // Don't watch the parents, because those are already included in `self.watched` (so
            // it'd be redundant), but also because this could deadlock, since we'd try to modify
            // `self.watched` while iterating over it (write lock overlapping with a read lock).
            let _ = start_watching_dir(&mut watching_state.notify_watcher, dir_path, root_path);
        }
    }

    /// Called when a new directory is found in a parent directory we're watching. Restores the
    /// watcher if we were previously watching it.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[instrument(skip_all, level = "trace")]
    pub async fn restore_if_watched(
        state: &RwLock<NonRecursiveState>,
        dir_path: &Path,
        root_path: &Path,
    ) -> Result<()> {
        // fast path: The root directory is always implicitly watched during
        // `DiskWatcher::start_watching`, we assume it is never deleted and never needs to be
        // restored.
        if dir_path == root_path {
            return Ok(());
        }

        // fast path: the directory isn't in `watched`, only take a read lock and bail out early
        {
            let guard = state.read().await;
            let NonRecursiveState::Watching(watching_state) = &*guard else {
                return Ok(());
            };
            if !watching_state.watched.contains(dir_path) {
                return Ok(());
            }
        }

        // slow path: re-watch the path
        let mut guard = state.write().await;
        let NonRecursiveState::Watching(watching_state) = &mut *guard else {
            return Ok(());
        };

        // watch the new directory
        start_watching_dir(&mut watching_state.notify_watcher, dir_path, root_path)?;

        // Also try to restore any watchers for children of this directory
        for child_path in watching_state.watched.iter_path_children(dir_path) {
            // Don't watch the parents -- see the comment on `restore_all_watched`
            start_watching_dir(&mut watching_state.notify_watcher, child_path, root_path)?;
        }
        Ok(())
    }

    /// Called when a file in `dir_path` or `dir_path` itself is read or written. Adds a new watcher
    /// if we're not already watching the directory.
    ///
    /// This should be called *before* reading a file to avoid a race condition.
    #[instrument(skip_all, level = "trace")]
    pub async fn ensure_watched(
        state: &RwLock<NonRecursiveState>,
        dir_path: &Path,
        root_path: &Path,
    ) -> Result<()> {
        // fast path: The root directory is always implicitly watched during
        // `DiskWatcher::start_watching`.
        if dir_path == root_path {
            return Ok(());
        }

        // fast path: the directory is already in `watched`, only take a read lock and bail out
        // early
        {
            let guard = state.read().await;
            let NonRecursiveState::Watching(watching_state) = &*guard else {
                return Ok(());
            };
            if watching_state.watched.contains(dir_path) {
                return Ok(());
            }
        }

        // slow path: watch the path
        let mut guard = state.write().await;
        let NonRecursiveState::Watching(watching_state) = &mut *guard else {
            return Ok(());
        };
        if watching_state.watched.insert(dir_path.to_path_buf()) {
            start_watching_dir_and_parents(watching_state, dir_path, root_path)?;
        }
        Ok(())
    }

    /// Private helper, assumes that `dir_path` has already been added to
    /// [`NonRecursiveWatchingState::watched`].
    ///
    /// This does not watch any of the parent directories. For that, use
    /// [`start_watching_dir_and_parents`]. Use this method when iterating over previously-watched
    /// values in `self.watching`.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn start_watching_dir(
        notify_watcher: &mut NotifyWatcher,
        dir_path: &Path,
        root_path: &Path,
    ) -> Result<()> {
        debug_assert_ne!(dir_path, root_path);

        match notify_watcher.watch(dir_path, notify::RecursiveMode::NonRecursive) {
            Ok(())
            | Err(notify::Error {
                // The path was probably deleted before we could process the event, but the parent
                // should still be watched. The codepaths that care about this either call
                // `start_watching_dir_and_parents` or handle the parents themselves.
                kind: notify::ErrorKind::PathNotFound,
                ..
            }) => Ok(()),
            Err(err) => {
                // ast-grep-ignore: no-context-format
                return Err(err).context(format!("Unable to watch {}", dir_path.display(),));
            }
        }
    }

    /// Private helper, assumes that `dir_path` has already been added to
    /// [`NonRecursiveWatchingState::watched`].
    ///
    /// Watches the given `dir_path` and every parent up to `root_path`. Parents must be recursively
    /// watched in case any of them change:
    /// https://docs.rs/notify/latest/notify/#parent-folder-deletion
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn start_watching_dir_and_parents(
        state: &mut NonRecursiveWatchingState,
        dir_path: &Path,
        root_path: &Path,
    ) -> Result<()> {
        let mut found_watched_ancestor = false;

        // NOTE: `Path::ancestors` yields ancestors from longest to shortest path.
        let dir_and_ancestor_paths: Vec<_> = [dir_path]
            .into_iter()
            .chain(
                dir_path
                    .ancestors()
                    // skip: `ancestors` includes `dir_path` itself, as well as the ancestors, but
                    // we only want to apply the `take_while` check to parents
                    .skip(1)
                    .take_while(|p| {
                        found_watched_ancestor = *p == root_path || state.watched.contains(*p);
                        !found_watched_ancestor
                    }),
            )
            .collect();

        if !found_watched_ancestor {
            // this should never happen, as we should eventually hit the `root_path`
            anyhow::bail!(
                "failed to find the fs root of {root_path:?} while watching {dir_path:?}"
            );
        }

        // Reverse the iterator: We want to start closest to the root and work towards `dir_path`
        // (opposite of `Path::ancestors`), to avoid a potential race condition if directories are
        // removed and re-added before we've watched their parent.
        for path in dir_and_ancestor_paths.into_iter().rev() {
            // this will silently ignore if the path is not found, expecting that we've watched the
            // parent directory
            start_watching_dir(&mut state.notify_watcher, path, root_path)?;
            state.watched.insert(path.to_owned());
        }

        Ok(())
    }
}

impl DiskWatcher {
    pub fn new(config: DiskWatcherConfig) -> Self {
        Self::new_with_ignored_paths(config, vec![])
    }

    /// Creates a watcher with ignored path components. Entries prefixed with
    /// `!node_modules/` are package-name regular expressions that opt dependencies back in.
    pub fn new_with_ignored_paths(config: DiskWatcherConfig, ignored_paths: Vec<RcStr>) -> Self {
        assert!(
            config.extended_batch_delay_duration >= config.batch_delay,
            "extended_batch_delay_duration must be at least batch_delay"
        );

        const NODE_MODULES_NEGATED_REGEX_PREFIX: &str = "!node_modules/";

        let mut paths = Vec::with_capacity(ignored_paths.len());
        let mut node_modules_regexes = Vec::new();
        for ignored in &ignored_paths {
            if let Some(pattern) = ignored.strip_prefix(NODE_MODULES_NEGATED_REGEX_PREFIX) {
                node_modules_regexes.push(RcStr::from(pattern));
            } else {
                paths.push(ignored.clone());
            }
        }

        let node_modules_regexes = if node_modules_regexes.is_empty() {
            None
        } else {
            RegexSet::new(node_modules_regexes.iter().map(RcStr::as_str))
                .ok()
                .map(Arc::new)
        };

        Self {
            state: State::new_stopped(config.resolve_recursive_mode()),
            config,
            ignored_path_config: Arc::new(ignored_paths),
            ignored_paths: Arc::new(paths),
            node_modules_regexes,
        }
    }

    /// Check if a path should be ignored based on configured ignore patterns
    fn should_ignore_path(&self, path: &Path) -> bool {
        should_ignore_path(
            path,
            &self.ignored_paths,
            self.node_modules_regexes.as_deref(),
        )
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    pub async fn start_watching<FsApi: DiskFileSystemWatcherApi>(fs: Arc<FsApi>) -> Result<()> {
        let watcher: &Self = fs.watcher();

        // read in the turbo-task context and before acquiring the lock
        let extended_batch_delay_matcher = match watcher.config.extended_batch_delay_matcher {
            Some(matcher) => Some(matcher.into_trait_ref().await?),
            None => None,
        };

        let state_guard = watcher.state.write().await;

        // bail out if we're already watching
        if let StateWriteGuard::Recursive(guard) = &state_guard
            && matches!(**guard, RecursiveState::Watching { .. })
        {
            return Ok(());
        } else if let StateWriteGuard::NonRecursive(guard) = &state_guard
            && matches!(**guard, NonRecursiveState::Watching(..))
        {
            return Ok(());
        }

        // Create a channel to receive the events.
        let (tx, rx) = channel();
        // Create a watcher object, delivering debounced events.
        // The notification back-end is selected based on the platform.
        let config = Config::default();
        // we should track and invalidate each part of a symlink chain ourselves in
        // turbo-tasks-fs
        let config = config.with_follow_symlinks(false);

        let mut notify_watcher = if let Some(poll_interval) = watcher.config.poll_interval {
            let config = config.with_poll_interval(poll_interval);
            NotifyWatcher::Polling(PollWatcher::new(tx, config)?)
        } else {
            NotifyWatcher::Recommended(RecommendedWatcher::new(tx, config)?)
        };

        // TOCTOU: we must watch `root_path` before calling any invalidators and setting up the
        // watchers in their associated functions
        let root_path = fs.root_path();
        notify_watcher.watch(
            root_path,
            notify::RecursiveMode::from(watcher.state.recursive_mode()),
        )?;

        // We need to invalidate all reads or writes that happened before watching. As a
        // side-effect, this will call `ensure_watched` again, setting up any watchers needed.
        //
        // Best is to start_watching before starting to read
        if watcher.config.report_invalidation_reason {
            let name = fs.name().clone();
            fs.invalidate_all_with_reason(|path| WatchStart {
                name: name.clone(),
                // this path is just used for display purposes
                path: RcStr::from(path.to_string_lossy()),
            });
        } else {
            fs.invalidate_all();
        }

        spawn_thread({
            let fs = fs.clone();
            move || Self::watch_thread(fs, rx, extended_batch_delay_matcher)
        });

        // Updating `self.state` is done last. If we panic while setting up the watcher, it'll
        // stay in the `Stopped` state.
        match state_guard {
            StateWriteGuard::Recursive(mut recursive) => {
                *recursive = RecursiveState::Watching {
                    _notify_watcher: notify_watcher,
                }
            }
            StateWriteGuard::NonRecursive(mut non_recursive) => {
                *non_recursive = NonRecursiveState::Watching(NonRecursiveWatchingState {
                    notify_watcher,
                    watched: BTreeSet::new(),
                })
            }
        };

        Ok(())
    }

    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    pub(crate) async fn start_watching<FsApi: DiskFileSystemWatcherApi>(
        fs: Arc<FsApi>,
    ) -> Result<()> {
        use crate::wasm_fs_offload;

        let watcher = fs.watcher();
        let state_guard = watcher.state.write().await;
        if let StateWriteGuard::Recursive(guard) = &state_guard
            && matches!(**guard, RecursiveState::Watching { .. })
        {
            return Ok(());
        } else if let StateWriteGuard::NonRecursive(guard) = &state_guard
            && matches!(**guard, NonRecursiveState::Watching(..))
        {
            return Ok(());
        }

        let root_path = fs.root_path().to_path_buf();
        let ignored_paths = watcher.ignored_paths.clone();
        let node_modules_regexes = watcher.node_modules_regexes.clone();
        let recursive_mode = watcher.state.recursive_mode();
        let report_invalidation_reason = watcher.config.report_invalidation_reason;
        let callback_fs = fs.clone();
        wasm_fs_offload::CLIENT
            .watch_dir(&root_path, true, move |event| {
                let Some(event) = filter_event_paths(event, |path| {
                    should_ignore_path(path, &ignored_paths, node_modules_regexes.as_deref())
                }) else {
                    return;
                };

                let mut batch = BatchedInvalidations::new(recursive_mode, false);
                if !batch.add_event(event) {
                    return;
                }

                let Some(turbo_tasks) = callback_fs.turbo_tasks() else {
                    // TurboTasks was dropped, stop watching
                    // wasm_fs_offload::CLIENT.stop_watching();
                    return;
                };
                let _guard = callback_fs.tokio_handle().enter();
                // `invalidation_lock.blocking_write()` hangs in the browser wasm runtime. The
                // OPFS callback executes the shared batch directly instead.
                batch.execute(
                    callback_fs.invalidator_map(),
                    callback_fs.dir_invalidator_map(),
                    |invalidation_reason_path, invalidator| {
                        invalidate(
                            &*callback_fs,
                            &*turbo_tasks,
                            report_invalidation_reason,
                            invalidation_reason_path,
                            invalidator,
                        )
                    },
                );
            })
            .await?;

        match state_guard {
            StateWriteGuard::Recursive(mut recursive) => {
                *recursive = RecursiveState::Watching {};
            }
            StateWriteGuard::NonRecursive(mut non_recursive) => {
                *non_recursive = NonRecursiveState::Watching(NonRecursiveWatchingState {
                    watched: BTreeSet::new(),
                });
            }
        }

        Ok(())
    }

    pub async fn stop_watching(&self) {
        match &self.state {
            State::Recursive(state) => *state.write().await = RecursiveState::Stopped,
            State::NonRecursive(state) => *state.write().await = NonRecursiveState::Stopped,
        }
        // thread will detect the stop because the channel is disconnected when `NotifyWatcher` is
        // dropped
    }

    /// Internal thread that processes the events from the watcher
    /// and invalidates the cache.
    ///
    /// Should only be called once from `start_watching`.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn watch_thread<FsApi: DiskFileSystemWatcherApi>(
        fs: Arc<FsApi>,
        rx: Receiver<notify::Result<notify::Event>>,
        extended_batch_delay_matcher: Option<TraitRef<Box<dyn DiskWatcherPathMatcher>>>,
    ) {
        let watcher: &Self = fs.watcher();
        let config = &watcher.config;
        let report_invalidation_reason = config.report_invalidation_reason;
        let mut batch = BatchedInvalidations::new(
            watcher.state.recursive_mode(),
            config.poll_interval.is_some(),
        );
        let mut schedule = BatchSchedule::new(config);

        'outer: loop {
            loop {
                match schedule.recv_event(&rx, &*fs, &batch) {
                    Ok(Ok(event)) => {
                        // TODO: We might benefit from some user-facing diagnostics if it rescans
                        // occur frequently (i.e. more than X times in Y minutes)
                        //
                        // You can test rescans on Linux by reducing the inotify queue to something
                        // really small:
                        //
                        // ```
                        // echo 3 | sudo tee /proc/sys/fs/inotify/max_queued_events
                        // ```
                        if event.need_rescan() {
                            let _lock = fs.invalidation_lock().blocking_write();

                            // flush the whole mpsc queue, we're about to rescan, we don't need to
                            // process any other update events that have already happened
                            while rx.try_recv().is_ok() {}

                            if let State::NonRecursive(non_recursive) = &watcher.state {
                                // we can't narrow this down to a smaller set of paths: Rescan
                                // events (at least when tested on
                                // Linux) come with no `paths`, and we use
                                // only one global `notify::Watcher` instance.
                                //
                                // TODO: Report diagnostics if an error happens
                                fs.tokio_handle().block_on(
                                    non_recursive_helpers::restore_all_watched_ignore_errors(
                                        non_recursive,
                                        fs.root_path(),
                                    ),
                                );
                            }

                            if report_invalidation_reason {
                                fs.invalidate_all_with_reason(|path| InvalidateRescan {
                                    // this path is just used for display purposes
                                    path: RcStr::from(path.to_string_lossy()),
                                });
                            } else {
                                fs.invalidate_all();
                            }

                            // no need to process the rest of the batch as we just
                            // invalidated everything
                            batch.clear();
                            schedule.reset();
                            break;
                        }

                        let Some(event) =
                            filter_event_paths(event, |path| watcher.should_ignore_path(path))
                        else {
                            continue;
                        };

                        // Any event that contributes to the batch keeps it open for another
                        // `batch_delay`. A path matching `extended_batch_delay_matcher` (e.g. a
                        // package-manager install target) keeps it open for
                        // `extended_batch_delay_duration` instead.
                        let mut delay = config.batch_delay;
                        if let Some(matcher) = &extended_batch_delay_matcher
                            && event.paths.iter().any(|path| matcher.match_path(path))
                        {
                            delay = delay.max(config.extended_batch_delay_duration);
                        }
                        if batch.add_event(event) {
                            schedule.extend(delay);
                        }
                    }
                    // Error raised by notify watcher itself
                    Ok(Err(notify::Error { kind, paths })) => {
                        println!("watch error ({paths:?}): {kind:?} ");

                        batch.add_error(paths, fs.root_path());
                        schedule.extend(config.batch_delay);
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        // the batch is complete: break out to invalidate the collected paths.
                        break;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        // Sender has been disconnected, which means DiskFileSystem has been dropped
                        // exit thread
                        break 'outer;
                    }
                }
            }

            // We need to start watching first before invalidating the changed paths...
            // This is only needed on platforms we don't do recursive watching on.
            if let State::NonRecursive(non_recursive) = &watcher.state {
                for path in batch.new_paths() {
                    // TODO: Report diagnostics if this error happens
                    let _ = fs
                        .tokio_handle()
                        .block_on(non_recursive_helpers::restore_if_watched(
                            non_recursive,
                            path,
                            fs.root_path(),
                        ));
                }
            }

            let Some(turbo_tasks) = fs.turbo_tasks() else {
                // TurboTasks was dropped, stop watching
                break 'outer;
            };
            let _guard = fs.tokio_handle().enter();

            let _lock = fs.invalidation_lock().blocking_write();
            batch.execute(
                fs.invalidator_map(),
                fs.dir_invalidator_map(),
                |invalidation_reason_path, invalidator| {
                    invalidate(
                        &*fs,
                        &*turbo_tasks,
                        report_invalidation_reason,
                        invalidation_reason_path,
                        invalidator,
                    )
                },
            );
        }
    }

    pub async fn ensure_watched_file(&self, path: &Path, root_path: &Path) -> Result<()> {
        // Watch the parent directory instead of the specified file, since directories also track
        // their immediate children (even in non-recursive mode), and we need to watch all the
        // parents anyways.
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        if let State::NonRecursive(non_recursive) = &self.state
            && let Some(dir_path) = path.parent()
        {
            non_recursive_helpers::ensure_watched(non_recursive, dir_path, root_path).await?;
        }
        Ok(())
    }

    pub async fn ensure_watched_dir(&self, dir_path: &Path, root_path: &Path) -> Result<()> {
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        if let State::NonRecursive(non_recursive) = &self.state {
            non_recursive_helpers::ensure_watched(non_recursive, dir_path, root_path).await?;
        }
        Ok(())
    }
}

bitflags! {
    /// Describes how a single path in a [`BatchedInvalidations`] should be invalidated. A path may
    /// carry any combination of these (accumulated across the events in a batch).
    struct InvalidationFlags: u8 {
        /// Invalidate exactly this path in the file-content invalidator map.
        const PATH = 1 << 0;
        /// Invalidate exactly this path in the directory-listing invalidator map.
        const PATH_DIR = 1 << 1;
        /// Invalidate this path and all of its children in the file-content invalidator map.
        const PATH_AND_CHILDREN = 1 << 2;
        /// Invalidate this path and all of its children in the directory-listing invalidator map.
        const PATH_AND_CHILDREN_DIR = 1 << 3;
    }
}

/// A set of deferred invalidations. Because one or more files may be updated many times in quick
/// succession, we don't want to perform invalidations until we think the filesystem has settled.
///
/// This avoids reading partially-written files which might generate transient errors, and reduces
/// CPU and memory usage by producing less wasted work.
///
/// Paths are stored once in a flag-keyed map, with a set of [`InvalidationFlags`] describing what
/// needs to happen for each, rather than in several separate sets. This avoids cloning each
/// `PathBuf` into multiple collections.
struct BatchedInvalidations {
    paths: FxIndexMap<Box<Path>, InvalidationFlags>,
    /// The most recently updated entry in [`Self::paths`].
    last_updated_index: Option<usize>,
    /// See [`Self::new_paths`]. Stored as [`None`] in recursive mode.
    new_paths: Option<FxHashSet<usize>>,
    /// Whether events are coming from [`PollWatcher`] instead of [`RecommendedWatcher`], which
    /// changes how a file content change is reported. See [`Self::is_content_change`].
    polling: bool,
}

impl BatchedInvalidations {
    fn new(recursive_mode: DiskWatcherRecursiveMode, polling: bool) -> Self {
        Self {
            paths: FxIndexMap::default(),
            last_updated_index: None,
            new_paths: match recursive_mode {
                DiskWatcherRecursiveMode::NonRecursive => Some(FxHashSet::default()),
                DiskWatcherRecursiveMode::Recursive => None,
            },
            polling,
        }
    }

    /// Whether a [`ModifyKind::Metadata`] event means the file's *contents* changed.
    ///
    /// Some backends don't report content changes as [`ModifyKind::Data`] at all, so we have to
    /// treat one specific metadata change per backend as a content change. Accepting these
    /// unconditionally would mean invalidating on every `chmod`/`touch` on the backends that do
    /// report `Data` properly.
    fn is_content_change(&self, kind: MetadataKind) -> bool {
        match kind {
            // `PollWatcher` detects changes by comparing mtimes, so a content change surfaces as a
            // write-time change. It only emits `Data` when `Config::with_compare_contents` is
            // enabled, which we don't do because hashing every watched file is too expensive.
            MetadataKind::WriteTime => self.polling,
            // fsevents does not always emit `kFSEventStreamEventFlagItemModified` for a content
            // change; sometimes it only emits `kFSEventStreamEventFlagItemInodeMetaMod`, which
            // notify maps to `MetadataKind::Any`. This causes redundant invalidations, but it's the
            // only way to reliably detect content changes there. Fix for PACK-2437.
            //
            // libuv does the same thing to trigger `UV_CHANGES`:
            // https://github.com/libuv/libuv/commit/73cf3600d75a5884b890a1a94048b8f3f9c66876
            //
            // inotify and ReadDirectoryChangesW both report content changes on their own, so
            // `MetadataKind::Any` there only ever means an actual attribute change.
            MetadataKind::Any => cfg!(target_os = "macos"),
            _ => false,
        }
    }

    fn clear(&mut self) {
        self.paths.clear();
        self.last_updated_index = None;
        if let Some(new_paths) = &mut self.new_paths {
            new_paths.clear();
        }
    }

    /// Records `index` as newly-created so its watch can be (re-)established. No-op in recursive
    /// watching mode.
    fn mark_new_path(&mut self, index: usize) {
        if let Some(new_paths) = &mut self.new_paths {
            new_paths.insert(index);
        }
    }

    /// Sets the `flags` for `path`. Returns the index that was modified.
    fn mark(&mut self, path: Cow<'_, Path>, flags: InvalidationFlags) -> usize {
        match self.paths.raw_entry_mut_v1().from_key(path.as_ref()) {
            RawEntryMut::Occupied(mut entry) => {
                *entry.get_mut() |= flags;
                entry.index()
            }
            RawEntryMut::Vacant(entry) => {
                let index = entry.index();
                entry.insert(path.into_owned().into_boxed_path(), flags);
                index
            }
        }
    }

    fn last_updated_path(&self) -> Option<&Path> {
        self.last_updated_index
            .and_then(|index| self.paths.get_index(index))
            .map(|(path, _)| &**path)
    }

    fn mark_parent_dir(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.mark(Cow::Borrowed(parent), InvalidationFlags::PATH_DIR);
        }
    }

    /// Iterates over the newly-created paths in this batch. In non-recursive watching mode, these
    /// must have their watches (re-)established before [`Self::execute`] is called (see the note
    /// there). Always empty in recursive mode.
    fn new_paths(&self) -> impl Iterator<Item = &Path> {
        self.new_paths
            .iter()
            .flatten()
            .map(|&index| self.paths.get_index(index).unwrap().0.as_ref())
    }

    /// Updates the batch to contain updated paths from the given event. Does not perform any
    /// invalidations.
    ///
    /// Returns whether the event contained relevant events.
    #[must_use]
    fn add_event(&mut self, event: Event) -> bool {
        let paths: Vec<PathBuf> = event.paths;
        let mut last_updated_index = None;
        match event.kind {
            EventKind::Modify(ModifyKind::Data(_)) => {
                for path in paths {
                    last_updated_index = Some(self.mark(Cow::Owned(path), InvalidationFlags::PATH));
                }
            }
            // Some backends (fsevents, polling) can report metadata events for file content changes
            EventKind::Modify(ModifyKind::Metadata(kind)) if self.is_content_change(kind) => {
                for path in paths {
                    last_updated_index = Some(self.mark(Cow::Owned(path), InvalidationFlags::PATH));
                }
            }
            EventKind::Create(_) => {
                for path in paths {
                    self.mark_parent_dir(&path);
                    let index = self.mark(
                        Cow::Owned(path),
                        InvalidationFlags::PATH_AND_CHILDREN
                            | InvalidationFlags::PATH_AND_CHILDREN_DIR,
                    );
                    self.mark_new_path(index);
                    last_updated_index = Some(index);
                }
            }
            EventKind::Remove(_) => {
                for path in paths {
                    self.mark_parent_dir(&path);
                    last_updated_index = Some(self.mark(
                        Cow::Owned(path),
                        InvalidationFlags::PATH_AND_CHILDREN
                            | InvalidationFlags::PATH_AND_CHILDREN_DIR,
                    ));
                }
            }
            // A single event emitted with both the `From` and `To` paths.
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                let [source, destination] = <[PathBuf; 2]>::try_from(paths)
                    .expect("RenameMode::Both event must contain exactly two paths");

                self.mark_parent_dir(&source);
                self.mark(Cow::Owned(source), InvalidationFlags::PATH_AND_CHILDREN);

                self.mark_parent_dir(&destination);
                let index = self.mark(
                    Cow::Owned(destination),
                    InvalidationFlags::PATH_AND_CHILDREN,
                );
                self.mark_new_path(index);
                last_updated_index = Some(index);
            }
            // We expect `RenameMode::Both` to cover most of the cases we need to invalidate,
            // but we also check other RenameModes to cover cases where notify couldn't match the
            // two rename events.
            EventKind::Any | EventKind::Modify(ModifyKind::Any | ModifyKind::Name(..)) => {
                for path in paths {
                    self.mark_parent_dir(&path);
                    last_updated_index = Some(self.mark(
                        Cow::Owned(path),
                        InvalidationFlags::PATH_AND_CHILDREN
                            | InvalidationFlags::PATH_AND_CHILDREN_DIR,
                    ));
                }
            }
            EventKind::Modify(ModifyKind::Metadata(..) | ModifyKind::Other)
            | EventKind::Access(_)
            | EventKind::Other => {}
        }
        if let Some(index) = last_updated_index {
            self.last_updated_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Updates the batch to invalidate paths associated with a watcher error.
    fn add_error(&mut self, paths: Vec<PathBuf>, root_path: &Path) {
        let flags = InvalidationFlags::PATH_AND_CHILDREN | InvalidationFlags::PATH_AND_CHILDREN_DIR;
        if paths.is_empty() {
            self.last_updated_index = Some(self.mark(Cow::Borrowed(root_path), flags));
        } else {
            for path in paths {
                self.last_updated_index = Some(self.mark(Cow::Owned(path), flags));
            }
        }
    }

    /// Performs all batched invalidations, calling `invalidate` once for each `(path, invalidator)`
    /// pair that needs to be invalidated, then clears the batch.
    ///
    /// In non-recursive watching mode, [`Self::new_paths`] must be processed (to (re-)establish
    /// watches) *before* calling this.
    ///
    /// For each path, a recursive invalidation subsumes an exact one, as
    /// [`extract_path_with_children`][OrderedPathMapExt::extract_path_with_children] removes the
    /// path itself in addition to its children.
    fn execute(
        &mut self,
        invalidator_map: &InvalidatorMap,
        dir_invalidator_map: &InvalidatorMap,
        invalidate: impl Fn(&Path, Invalidator),
    ) {
        for (map, exact_flag, recursive_flag) in [
            (
                invalidator_map,
                InvalidationFlags::PATH,
                InvalidationFlags::PATH_AND_CHILDREN,
            ),
            (
                dir_invalidator_map,
                InvalidationFlags::PATH_DIR,
                InvalidationFlags::PATH_AND_CHILDREN_DIR,
            ),
        ] {
            let mut map = map.lock().unwrap();
            for (path, flags) in &self.paths {
                if flags.contains(recursive_flag) {
                    for (_, invalidators) in map.extract_path_with_children(path) {
                        for invalidator in invalidators {
                            invalidate(path, invalidator);
                        }
                    }
                } else if flags.contains(exact_flag)
                    && let Some(invalidators) = map.remove(&**path)
                {
                    for invalidator in invalidators {
                        invalidate(path, invalidator);
                    }
                }
            }
        }
        self.clear();
    }
}

/// Removes ignored paths from an event and preserves the rename invariant expected by
/// [`BatchedInvalidations::add_event`]. A `RenameMode::Both` event may cross an ignored-path
/// boundary, leaving only one visible endpoint. Treat that as an unmatched rename so the visible
/// path and its children are still invalidated instead of panicking on the missing endpoint.
fn filter_event_paths(
    mut event: Event,
    mut should_ignore: impl FnMut(&Path) -> bool,
) -> Option<Event> {
    event.paths.retain(|path| !should_ignore(path));
    if event.paths.is_empty() {
        return None;
    }
    if matches!(
        event.kind,
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
    ) && event.paths.len() != 2
    {
        event.kind = EventKind::Modify(ModifyKind::Name(RenameMode::Any));
    }
    Some(event)
}

fn should_ignore_path(
    path: &Path,
    ignored_paths: &[RcStr],
    node_modules_regexes: Option<&RegexSet>,
) -> bool {
    if ignored_paths.is_empty() {
        return false;
    }

    let watch_node_module = node_modules_regexes
        .map(|regexes| matches_node_module_package(path, regexes))
        .unwrap_or(false);

    path.components().any(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        let Some(name) = name.to_str() else {
            return false;
        };

        ignored_paths.iter().any(|ignored| {
            ignored.as_str() == name && (name != "node_modules" || !watch_node_module)
        })
    })
}

fn matches_node_module_package(path: &Path, regexes: &RegexSet) -> bool {
    let mut components = path.components().filter_map(|component| match component {
        Component::Normal(name) => name.to_str(),
        _ => None,
    });
    let mut matches = false;

    while let Some(component) = components.next() {
        if component != "node_modules" {
            continue;
        }
        matches = false;
        let Some(first) = components.next() else {
            continue;
        };
        if first.starts_with('@') {
            if let Some(second) = components.next() {
                matches = regexes.is_match(&format!("{first}/{second}"));
            }
        } else {
            matches = regexes.is_match(first);
        }
    }

    matches
}

#[instrument(
    parent = None,
    level = "info",
    name = "file change",
    skip_all,
    fields(name = %invalidation_reason_path.display())
)]
fn invalidate(
    inner: &impl DiskFileSystemWatcherApi,
    turbo_tasks: &dyn TurboTasksApi,
    report_invalidation_reason: bool,
    invalidation_reason_path: &Path,
    invalidator: Invalidator,
) {
    if report_invalidation_reason
        && let Some(path) =
            format_absolute_fs_path(invalidation_reason_path, inner.name(), inner.root_path())
    {
        invalidator.invalidate_with_reason(turbo_tasks, WatchChange { path });
        return;
    }
    invalidator.invalidate(turbo_tasks);
}

/// Invalidation was caused by a watcher rescan event. This will likely invalidate *every* watched
/// file.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InvalidateRescan {
    path: RcStr,
}

impl InvalidationReason for InvalidateRescan {
    fn kind(&self) -> Option<StaticOrArc<dyn InvalidationReasonKind>> {
        Some(StaticOrArc::Static(&INVALIDATE_RESCAN_KIND))
    }
}

impl fmt::Display for InvalidateRescan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} in filesystem invalidated", self.path)
    }
}

/// [Invalidation kind][InvalidationReasonKind] for [`InvalidateRescan`].
#[derive(PartialEq, Eq, Hash)]
struct InvalidateRescanKind;

static INVALIDATE_RESCAN_KIND: InvalidateRescanKind = InvalidateRescanKind;

impl InvalidationReasonKind for InvalidateRescanKind {
    fn fmt(
        &self,
        reasons: &FxIndexSet<StaticOrArc<dyn InvalidationReason>>,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let first_reason: &dyn InvalidationReason = &*reasons[0];
        write!(
            f,
            "{} items in filesystem invalidated due to notify::Watcher rescan event ({}, ...)",
            reasons.len(),
            (first_reason as &dyn Any)
                .downcast_ref::<InvalidateRescan>()
                .unwrap()
                .path
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{Instant, SystemTime},
    };

    use rstest::rstest;
    use turbo_tasks::TurboTasks;
    use turbo_tasks_backend::{BackendOptions, TurboTasksBackend, noop_backing_storage};

    use super::*;
    use crate::watcher::mock_fs_api::MockFileSystem;

    /// Polls [`tracked_read`] until it has executed more than `previous_runs` times, i.e. until the
    /// watcher has invalidated it.
    async fn wait_for_rerun(fs: &Arc<MockFileSystem>, path: &Path, previous_runs: u64) {
        const WATCH_TIMEOUT: Duration = Duration::from_secs(5);
        let deadline = Instant::now() + WATCH_TIMEOUT;
        loop {
            if fs.tracked_read_strongly_consistent(path).await > previous_runs {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the watcher did not invalidate {path:?} within {WATCH_TIMEOUT:?}",
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Backdates `path`'s mtime so that a [`PollWatcher`] can see the write that follows it to
    /// avoid mtime truncation issues.
    fn backdate(path: &Path) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(10))
            .unwrap();
    }

    /// `recursive_mode` is set explicitly rather than left to the platform default so that both
    /// watching strategies are covered on every host. `TURBO_TASKS_FORCE_WATCH_MODE` still
    /// overrides it, collapsing these into two cases.
    #[rstest]
    #[case::native_recursive(None, DiskWatcherRecursiveMode::Recursive)]
    #[case::native_non_recursive(None, DiskWatcherRecursiveMode::NonRecursive)]
    #[case::polling_recursive(Some(Duration::from_millis(20)), DiskWatcherRecursiveMode::Recursive)]
    #[case::polling_non_recursive(
        Some(Duration::from_millis(20)),
        DiskWatcherRecursiveMode::NonRecursive
    )]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn watches_file_and_directory_changes(
        #[case] poll_interval: Option<Duration>,
        #[case] recursive_mode: DiskWatcherRecursiveMode,
    ) {
        let tt = TurboTasks::new(TurboTasksBackend::new(
            BackendOptions::default(),
            noop_backing_storage(),
        ));
        tt.run_once(async move {
            let fs = MockFileSystem::new(DiskWatcherConfig {
                recursive_mode: Some(recursive_mode),
                poll_interval,
                report_invalidation_reason: true,
                ..Default::default()
            });
            let sub_dir = fs.root_path.join("sub");
            let file_path = sub_dir.join("file.txt");
            fs::create_dir(&sub_dir).unwrap();
            fs::write(&file_path, "initial").unwrap();
            backdate(&file_path);

            DiskWatcher::start_watching(fs.clone()).await?;

            // the initial reads register the invalidators that the watcher will later fire
            assert_eq!(fs.tracked_read_strongly_consistent(&file_path).await, 1);
            assert_eq!(fs.tracked_read_strongly_consistent(&sub_dir).await, 1);

            // reading again without touching the filesystem must not re-run anything
            assert_eq!(fs.tracked_read_strongly_consistent(&file_path).await, 1);
            assert_eq!(fs.tracked_read_strongly_consistent(&sub_dir).await, 1);

            // modifying a file invalidates the task that read that file
            fs::write(&file_path, "updated")?;
            wait_for_rerun(&fs, &file_path, 1).await;

            // creating a file invalidates the task that listed the containing directory
            let dir_runs = fs.tracked_read_strongly_consistent(&sub_dir).await;
            fs::write(sub_dir.join("new.txt"), "new")?;
            wait_for_rerun(&fs, &sub_dir, dir_runs).await;

            fs.watcher.stop_watching().await;
            anyhow::Ok(())
        })
        .await
        .unwrap();
    }

    #[test]
    fn filters_rename_events_without_breaking_both_invariant() {
        let watcher = DiskWatcher::new_with_ignored_paths(
            DiskWatcherConfig::default(),
            vec!["node_modules".into()],
        );
        let visible_path = PathBuf::from("project/src/old.js");
        let ignored_path = PathBuf::from("project/node_modules/pkg/new.js");

        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(visible_path.clone())
            .add_path(ignored_path.clone());
        let event = filter_event_paths(event, |path| watcher.should_ignore_path(path)).unwrap();

        assert_eq!(event.paths.as_slice(), std::slice::from_ref(&visible_path));
        assert!(matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::Any))
        ));

        let mut batch = BatchedInvalidations::new(DiskWatcherRecursiveMode::Recursive, false);
        assert!(batch.add_event(event));
        let flags = batch.paths.get(visible_path.as_path()).unwrap();
        assert!(flags.contains(InvalidationFlags::PATH_AND_CHILDREN));
        assert!(flags.contains(InvalidationFlags::PATH_AND_CHILDREN_DIR));

        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(PathBuf::from("project/src/old.js"))
            .add_path(PathBuf::from("project/src/new.js"));
        let event = filter_event_paths(event, |path| watcher.should_ignore_path(path)).unwrap();
        assert!(matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        ));

        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(ignored_path.clone())
            .add_path(ignored_path.join("nested"));
        assert!(filter_event_paths(event, |path| watcher.should_ignore_path(path)).is_none());
    }

    #[test]
    fn watches_matching_node_modules_packages() {
        let watcher = DiskWatcher::new_with_ignored_paths(
            DiskWatcherConfig::default(),
            vec![
                "node_modules".into(),
                "!node_modules/rc-.*".into(),
                "!node_modules/.*cssinjs.*".into(),
                "!node_modules/@rc-component/.*".into(),
            ],
        );

        for path in [
            "project/node_modules/rc-util/index.js",
            "project/node_modules/@ant-design/cssinjs/index.js",
            "project/node_modules/@rc-component/trigger/index.js",
            "project/node_modules/.pnpm/rc-util@5.0.0/node_modules/rc-util/index.js",
        ] {
            assert!(!watcher.should_ignore_path(Path::new(path)));
        }
    }

    #[test]
    fn ignores_non_matching_node_modules_packages() {
        let watcher = DiskWatcher::new_with_ignored_paths(
            DiskWatcherConfig::default(),
            vec!["node_modules".into(), "!node_modules/rc-.*".into()],
        );

        assert!(watcher.should_ignore_path(Path::new("project/node_modules/react/index.js")));
        assert!(watcher.should_ignore_path(Path::new(
            "project/node_modules/rc-util/node_modules/react/index.js",
        )));
        assert!(watcher.should_ignore_path(Path::new(
            "project/node_modules/rc-util/node_modules/@scope",
        )));
        assert!(!watcher.should_ignore_path(Path::new("project/src/index.js")));
    }

    #[test]
    fn ignores_invalid_node_modules_regexes() {
        let watcher = DiskWatcher::new_with_ignored_paths(
            DiskWatcherConfig::default(),
            vec!["node_modules".into(), "!node_modules/[".into()],
        );

        assert!(watcher.should_ignore_path(Path::new("project/node_modules/rc-util/index.js",)));
    }
}
