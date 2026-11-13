//! Source: original
//! How many lanes each kind of kernel runs at, per target: the one home of these numbers. `gates perf` holds every width
//! above 1 to a clear gain over width 1, natively by instruction count and in the browser's simd128 build by time, and a
//! kernel that cannot show one runs at width 1 (L29, the threads and lanes topic). The figures below are from `maths`,
//! on the baseline x86-64 build (SSE2) and in Chromium; `just perf` prints them again.

/// Comparison masks and selection, as box tests use them: 26% of width 1's instructions at 8, and 5.7 times as fast
/// as width 1 at 4 with simd128.
pub const MASK: usize = if cfg!(target_family = "wasm") { 4 } else { 8 };

/// The predicate filter, interval arithmetic on four decisions at once (P6): 48% of width 1's instructions, and 1.9
/// times as fast with simd128.
pub const FILTER: usize = 4;

/// Analytic kernels, `sin_cos` and `atan2`, as the analytic surfaces evaluate and invert them in bulk: 72% and 73% of
/// width 1's instructions at 4, and 1.9 and 1.4 times as fast with simd128. At 2 on x86-64, `atan2` took more
/// instructions than at width 1.
pub const ANALYTIC: usize = 4;

/// Spline surfaces at many points of one span, as a tessellator asks for them, where the span's knots and poles are
/// read once for every lane: 49% of width 1's instructions at 4 on a bicubic's points and first derivatives, and 2.05
/// times as fast with simd128.
pub const SPLINE: usize = 4;
