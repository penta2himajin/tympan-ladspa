//! Realtime-safe primitives.
//!
//! This module hosts types and data structures safe to use from a
//! LADSPA plugin's `run()` callback. The realtime path runs on the
//! host audio thread; per `CLAUDE.md` § Prohibitions and ADR 0005,
//! code reachable from `run()` must be allocation-free, lock-free,
//! and free of blocking system calls.
//!
//! Current public surface:
//!
//! - [`RealtimeContext`] — zero-sized type-level marker witnessing
//!   that the caller is on the realtime thread.
//! - [`ring`] — lock-free single-producer / single-consumer ring
//!   buffer. Canonical use case: capturing diagnostic events from
//!   `run` for a non-realtime drainer to consume.
//!
//! [`Plugin`]: crate

mod context;
pub mod ring;

pub use context::RealtimeContext;
