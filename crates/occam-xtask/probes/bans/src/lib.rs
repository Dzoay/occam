//! One module per banned pattern. Never built in place: `probes::clippy_rejects_every_banned_pattern` copies this crate
//! into a scratch workspace with the repository's `clippy.toml` and `[workspace.lints]` and runs clippy as the gate does.
//! Every line carrying a `fires:` comment must get exactly the diagnostic it names, and no other line may get one.
#![allow(missing_docs, reason = "probe items are not API")]
#![allow(clippy::must_use_candidate, clippy::missing_panics_doc, reason = "probe items are not API")]

pub mod arithmetic_side_effects;
pub mod dbg_macro;
pub mod disallowed_any;
pub mod disallowed_float_methods;
pub mod disallowed_hash_map;
pub mod disallowed_hash_set;
pub mod expect_used;
pub mod indexing_slicing;
pub mod panic;
pub mod print_stderr;
pub mod print_stdout;
pub mod string_slice;
pub mod todo;
pub mod too_many_lines;
pub mod unimplemented;
pub mod unreachable;
pub mod unsafe_code;
pub mod unwrap_used;
