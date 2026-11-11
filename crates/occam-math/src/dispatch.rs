//! Source: original
//! [`dispatch`]: a lane kernel compiled for the widest instruction level the machine has, found at run time, with no
//! `unsafe` and no `target-cpu` build (P4). Every level gives the same bits, because nothing fuses, so
//! `docs/DETERMINISM.md` rule 5 stands. `fearless_simd` is named here and nowhere else in the kernel.

use fearless_simd::Simd;

/// An instruction level a kernel can be compiled for.
#[derive(Clone, Copy, Debug)]
pub struct Level(fearless_simd::Level);

impl Level {
    /// The widest level this machine has. Found once, then remembered.
    #[must_use]
    pub fn detect() -> Self {
        Self(fearless_simd::Level::new())
    }

    /// The level every build of this target has: SSE2 on x86-64, and on wasm32 simd128 or none, as built.
    #[must_use]
    pub const fn baseline() -> Self {
        Self(fearless_simd::Level::baseline())
    }

    /// Every level this machine has, widest first, so that `gates determinism` can compare their bits. The first is
    /// [`Level::detect`]'s.
    #[must_use]
    pub fn available() -> Vec<Self> {
        let widest = Self::detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let levels = {
            let found = [
                widest.0.as_avx512().map(Simd::level),
                widest.0.as_avx2().map(Simd::level),
                widest.0.as_sse4_2().map(Simd::level),
                widest.0.as_sse2().map(Simd::level),
            ];
            found.into_iter().flatten().map(Self).collect()
        };
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let levels = vec![widest];
        levels
    }

    /// The level among [`Level::available`] with this [name](Level::name).
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::available().into_iter().find(|level| level.name() == name)
    }

    /// A short name: `avx512`, `avx2`, `sse4.2` or `sse2` on x86-64, `simd128` on wasm32 built with it, `neon` on
    /// aarch64, and `fallback` where there is no vector level.
    #[must_use]
    pub fn name(self) -> &'static str {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let named = [
            (self.0.as_avx512().is_some(), "avx512"),
            (self.0.as_avx2().is_some(), "avx2"),
            (self.0.as_sse4_2().is_some(), "sse4.2"),
            (self.0.as_sse2().is_some(), "sse2"),
        ];
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        let named = [(self.0.as_wasm_simd128().is_some(), "simd128")];
        #[cfg(target_arch = "aarch64")]
        let named = [(self.0.as_neon().is_some(), "neon")];
        #[cfg(not(any(
            target_arch = "x86",
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        let named: [(bool, &str); 0] = [];
        named.into_iter().find_map(|(has, name)| has.then_some(name)).unwrap_or("fallback")
    }
}

/// A lane kernel: plain [`Real`](crate::Real) arithmetic, run by [`dispatch`].
///
/// `run`, and everything it calls that computes, must be `#[inline(always)]`. Anything that is not inlined is compiled
/// for the baseline, which gives the same bits but not the speed (P4). A trait rather than a closure, because a closure
/// cannot carry that attribute on stable Rust.
pub trait LaneKernel {
    /// What the kernel gives.
    type Output;

    /// Runs the kernel.
    fn run(self) -> Self::Output;
}

/// Runs `kernel` compiled for `level`. The bits are the same at every level; only the speed differs.
pub fn dispatch<K: LaneKernel>(level: Level, kernel: K) -> K::Output {
    fearless_simd::dispatch!(level.0, _simd => kernel.run())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Square(f64);

    impl LaneKernel for Square {
        type Output = f64;

        #[inline(always)]
        fn run(self) -> f64 {
            self.0 * self.0
        }
    }

    #[test]
    fn dispatch_runs_the_kernel_at_every_level() {
        for level in Level::available() {
            assert_eq!(dispatch(level, Square(1.5)), 2.25, "{}", level.name());
        }
    }

    #[test]
    fn the_levels_are_widest_first_and_end_at_the_baseline() {
        let names: Vec<&str> = Level::available().into_iter().map(Level::name).collect();
        assert_eq!(names.first().copied(), Some(Level::detect().name()));
        assert_eq!(names.last().copied(), Some(Level::baseline().name()));
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(unique, names, "no level twice");
    }

    #[test]
    fn a_level_is_found_by_its_name_and_only_if_this_machine_has_it() {
        for level in Level::available() {
            assert_eq!(Level::named(level.name()).map(Level::name), Some(level.name()));
        }
        assert!(Level::named("no-such-level").is_none());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn on_x86_64_the_baseline_is_sse2_and_every_level_down_to_it_is_offered() {
        assert_eq!(Level::baseline().name(), "sse2");
        let names: Vec<&str> = Level::available().into_iter().map(Level::name).collect();
        let order = ["avx512", "avx2", "sse4.2", "sse2"];
        let first = order.iter().position(|name| *name == names[0]).unwrap();
        assert_eq!(names, order[first..], "each narrower level follows the widest");
    }

    #[cfg(target_family = "wasm")]
    #[test]
    fn on_wasm32_the_one_level_is_the_build_s() {
        let want = if cfg!(target_feature = "simd128") { "simd128" } else { "fallback" };
        let names: Vec<&str> = Level::available().into_iter().map(Level::name).collect();
        assert_eq!(names, [want]);
    }
}
