//! Realtime-safe primitives.
//!
//! This module hosts types and (eventually) data structures that are
//! safe to use from a LADSPA plugin's `run()` callback. The realtime
//! path runs on the host audio thread; per `CLAUDE.md` § Prohibitions
//! and ADR 0005, code reachable from `run()` must be allocation-free,
//! lock-free, and free of blocking system calls.
//!
//! The current public surface is the [`RealtimeContext`] marker. As
//! the framework grows, this module will also gain:
//!
//! - A lock-free single-producer / single-consumer ring buffer for
//!   diagnostic events drained off-thread.
//! - Atomic state-machine helpers for the LADSPA plugin lifecycle
//!   (`instantiated` → `activated` → `running` → `deactivated`).
//!
//! Neither is required to support the minimum viable [`Plugin`]
//! trait, so they are intentionally deferred to follow-up PRs.
//!
//! [`Plugin`]: crate

mod context;

pub use context::RealtimeContext;
