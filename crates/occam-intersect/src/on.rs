//! Source: original
//! Where a pair's own work runs: on the calling thread, with its two surfaces borrowed, or one item a task through
//! [`Exec`], with them shared. Either way the results come back in item order, so the bits do not depend on which, nor
//! on the number of threads (L6).

use std::sync::Arc;

use occam_geom::Surface;
use occam_math::{Exec, Grain};

use crate::methods::SurfacePair;

/// A pair of surfaces and where work on it runs.
#[derive(Clone, Copy)]
pub(crate) enum On<'s> {
    /// On the calling thread.
    Here(&'s dyn Surface, &'s dyn Surface),
    /// Through `Exec`, the surfaces shared with its tasks.
    Shared(&'s Exec, &'s SurfacePair),
}

impl On<'_> {
    /// The two surfaces, the first given first.
    pub(crate) fn surfaces(&self) -> (&dyn Surface, &dyn Surface) {
        match self {
            Self::Here(a, b) => (*a, *b),
            Self::Shared(_, (a, b)) => (a.as_ref(), b.as_ref()),
        }
    }

    /// `f` of the two surfaces and each of `items`, in item order. Shared, the items are split into tasks by `grain`.
    pub(crate) fn map<T, R, F>(&self, items: Arc<[T]>, grain: Grain, f: F) -> Vec<R>
    where
        T: Send + Sync + 'static,
        R: Send + 'static,
        F: Fn(&dyn Surface, &dyn Surface, &T) -> R + Send + Sync + 'static,
    {
        match self {
            Self::Here(a, b) => items.iter().map(|item| f(*a, *b, item)).collect(),
            Self::Shared(exec, (a, b)) => {
                let (a, b, count) = (Arc::clone(a), Arc::clone(b), items.len());
                let found =
                    exec.map(count, grain, move |index| items.get(index).map(|item| f(a.as_ref(), b.as_ref(), item)));
                found.into_iter().flatten().collect()
            }
        }
    }
}
