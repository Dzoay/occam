//! Source: original
//! [`Exec`]: the one way the kernel runs in parallel (P11). Threads go across items and lanes inside each task. Results
//! come back in index order and nothing is reduced, so no result depends on how the work was split or which thread
//! finished first (`docs/DETERMINISM.md` rule 4). `rayon` is named here and nowhere else in the kernel.

use std::num::NonZeroUsize;
use std::ops::Range;
#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;

use occam_report::{Code, Fault, NamedValue, Remedy, RemedyKind, Report};

/// How work is split into tasks. It changes the speed, never the bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grain {
    /// Left to the runner, for cheap items such as box tests (P11: 3.94 times on six threads at 100 ns an item).
    Auto,
    /// One item a task, for items of 10 µs and more: a face, a pair, a region (P2, P9, P11).
    Each,
}

/// The execution layer: an ordered map over `rayon` natively, and serial on wasm32. A concrete type, so that no kernel
/// function is generic over its runner and a swap to another runner stays inside this type.
#[derive(Clone, Debug)]
pub struct Exec {
    runner: Runner,
}

#[derive(Clone, Debug)]
enum Runner {
    /// The one-thread baseline, and the runner inside each browser worker (P10).
    Serial,
    #[cfg(not(target_family = "wasm"))]
    Pool(Arc<rayon::ThreadPool>),
}

/// The report for a pool of `count` threads that could not start, with the remedy that always works.
fn unavailable(count: NonZeroUsize) -> Report {
    let threads = u32::try_from(count.get()).unwrap_or(u32::MAX);
    Report::new(Code::ThreadsUnavailable, Fault::Internal)
        .with_value(NamedValue::count("threads", threads))
        .with_remedy(Remedy::new(RemedyKind::UseThreads).with_value(NamedValue::count("threads", 1)))
}

impl Exec {
    /// Everything on the calling thread.
    #[must_use]
    pub const fn serial() -> Self {
        Self { runner: Runner::Serial }
    }

    /// `count` threads. One thread is [`Exec::serial`], which starts none. On wasm32 every count is serial, because the
    /// kernel runs single-threaded inside each worker (P10).
    ///
    /// # Errors
    ///
    /// `exec.threads-unavailable` if the threads cannot be started, with the remedy `use-threads` at one thread.
    pub fn threads(count: NonZeroUsize) -> Result<Self, Report> {
        #[cfg(not(target_family = "wasm"))]
        if count.get() > 1 {
            return Self::pool(count, rayon::ThreadPoolBuilder::new().num_threads(count.get()).build());
        }
        #[cfg(target_family = "wasm")]
        let _ = count;
        Ok(Self::serial())
    }

    /// A pool from what building it gave.
    #[cfg(not(target_family = "wasm"))]
    fn pool(
        count: NonZeroUsize,
        built: Result<rayon::ThreadPool, rayon::ThreadPoolBuildError>,
    ) -> Result<Self, Report> {
        built.map(|pool| Self { runner: Runner::Pool(Arc::new(pool)) }).map_err(|_| unavailable(count))
    }

    /// How many threads the work is shared among.
    #[must_use]
    pub fn thread_count(&self) -> usize {
        match &self.runner {
            Runner::Serial => 1,
            #[cfg(not(target_family = "wasm"))]
            Runner::Pool(pool) => pool.current_num_threads(),
        }
    }

    /// `f` over `0..n`, the results in index order whatever order they finished in. There is no reduction, so nothing
    /// can depend on how the work was split (P11: `rayon`'s float `sum` gave 7 bit patterns in 8 runs). The closure is
    /// `'static`, which keeps a swap to a small pool of our own possible.
    pub fn map<R, F>(&self, n: usize, grain: Grain, f: F) -> Vec<R>
    where
        R: Send + 'static,
        F: Fn(usize) -> R + Send + Sync + 'static,
    {
        match &self.runner {
            Runner::Serial => (0..n).map(f).collect(),
            #[cfg(not(target_family = "wasm"))]
            Runner::Pool(pool) => {
                use rayon::prelude::*;
                let most = match grain {
                    Grain::Auto => usize::MAX,
                    Grain::Each => 1,
                };
                pool.install(|| (0..n).into_par_iter().with_max_len(most).map(f).collect())
            }
        }
    }

    /// A batch of `len` items in chunks of `chunk`, each chunk one task and one lane group, the results concatenated in
    /// item order. The boundaries depend on `chunk` alone, never on the thread count. P6's predicate batches run here at
    /// 256 to 1,024 items a chunk.
    pub fn map_chunks<R, F>(&self, len: usize, chunk: NonZeroUsize, f: F) -> Vec<R>
    where
        R: Send + 'static,
        F: Fn(Range<usize>) -> Vec<R> + Send + Sync + 'static,
    {
        let size = chunk.get();
        let parts = self.map(len.div_ceil(size), Grain::Each, move |index| {
            let start = index.saturating_mul(size);
            f(start..start.saturating_add(size).min(len))
        });
        parts.into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;

    use occam_report::followed_remedy;

    use super::*;

    fn count(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    fn runners() -> Vec<Exec> {
        let mut found = vec![Exec::serial()];
        found.extend([1, 2, 7, 64].map(|n| Exec::threads(count(n)).unwrap()));
        found
    }

    /// Early items take longest, so on several threads they finish last.
    fn slow_first(index: usize) -> usize {
        if index < 4 {
            thread::sleep(Duration::from_millis(20));
        }
        index.wrapping_mul(index)
    }

    #[test]
    fn map_returns_results_in_index_order_whatever_order_they_finished_in() {
        let want: Vec<usize> = (0..200).map(|index: usize| index.wrapping_mul(index)).collect();
        for exec in runners() {
            for grain in [Grain::Auto, Grain::Each] {
                assert_eq!(exec.map(200, grain, slow_first), want, "{} threads, {grain:?}", exec.thread_count());
            }
        }
    }

    #[test]
    fn map_chunks_has_boundaries_that_depend_only_on_the_chunk_size() {
        for exec in runners() {
            let ranges = exec.map_chunks(10, count(4), |range| vec![(range.start, range.end)]);
            assert_eq!(ranges, [(0, 4), (4, 8), (8, 10)], "{} threads", exec.thread_count());
            let items = exec.map_chunks(10, count(3), |range| range.map(|index| index.wrapping_mul(2)).collect());
            assert_eq!(items, (0..10).map(|index: usize| index.wrapping_mul(2)).collect::<Vec<_>>());
        }
    }

    #[test]
    fn nothing_to_do_gives_nothing() {
        for exec in runners() {
            assert_eq!(exec.map(0, Grain::Auto, |index| index), Vec::<usize>::new());
            assert_eq!(exec.map_chunks(0, count(4), Iterator::collect::<Vec<_>>), Vec::<usize>::new());
        }
    }

    #[test]
    fn one_thread_is_serial_and_a_pool_has_the_threads_asked_for() {
        assert_eq!(Exec::serial().thread_count(), 1);
        assert_eq!(Exec::threads(count(1)).unwrap().thread_count(), 1);
        #[cfg(not(target_family = "wasm"))]
        assert_eq!(Exec::threads(count(3)).unwrap().thread_count(), 3);
        #[cfg(target_family = "wasm")]
        assert_eq!(Exec::threads(count(3)).unwrap().thread_count(), 1);
    }

    #[test]
    fn one_thread_is_the_calling_thread() {
        let caller = thread::current().id();
        let ran_on = Exec::threads(count(1)).unwrap().map(4, Grain::Each, |_| thread::current().id());
        assert!(ran_on.iter().all(|id| *id == caller), "one thread starts no pool");
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn a_pool_shares_the_items_among_its_threads() {
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let record = std::sync::Arc::clone(&seen);
        Exec::threads(count(4)).unwrap().map(64, Grain::Each, move |_| {
            thread::sleep(Duration::from_millis(2));
            let mut ids = record.lock().unwrap();
            let id = thread::current().id();
            if !ids.contains(&id) {
                ids.push(id);
            }
        });
        assert!(seen.lock().unwrap().len() > 1);
    }

    /// What building a pool gives when the machine refuses to start its threads.
    #[cfg(not(target_family = "wasm"))]
    fn refused() -> Result<Exec, Report> {
        let builder = rayon::ThreadPoolBuilder::new().num_threads(4);
        let built = builder.spawn_handler(|_| Err(std::io::Error::other("refused"))).build();
        Exec::pool(count(4), built)
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn threads_that_cannot_start_are_reported_with_how_many() {
        let report = refused().unwrap_err();
        assert_eq!(report.code, Code::ThreadsUnavailable);
        assert_eq!(report.value("threads").and_then(NamedValue::number), Some(4.0));
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn following_the_remedy_succeeds() {
        followed_remedy!(refused().unwrap_err(), Code::ThreadsUnavailable, RemedyKind::UseThreads, |remedy| {
            let threads = remedy.value("threads").and_then(NamedValue::number).unwrap_or(0.0);
            assert_eq!(threads, 1.0);
            let exec = Exec::threads(count(1))?;
            assert_eq!(exec.map(3, Grain::Auto, |index| index), [0, 1, 2]);
            Ok(exec)
        });
    }
}
