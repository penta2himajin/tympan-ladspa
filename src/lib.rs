//! A safe Rust framework for writing LADSPA plugins.
//!
//! Implementation is in progress. The public API surface described in
//! [`docs/architecture.md`](https://github.com/penta2himajin/tympan-ladspa/blob/main/docs/architecture.md)
//! is being assembled bottom-up:
//!
//! - [`raw`] — low-level FFI mirroring `ladspa.h`.
//! - [`realtime`] — primitives safe to call from the host's realtime
//!   audio thread.
//!
//! The high-level `Plugin` trait, `Ports`, and the `plugin_entry!`
//! macro are not yet built.
//!
//! See `docs/decisions/` for the architectural decisions that constrain
//! this crate.

pub mod raw;
pub mod realtime;
