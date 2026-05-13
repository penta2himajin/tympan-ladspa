//! A safe Rust framework for writing LADSPA plugins.
//!
//! Implementation is in progress. The public API surface described in
//! [`docs/architecture.md`](https://github.com/penta2himajin/tympan-ladspa/blob/main/docs/architecture.md)
//! is assembled bottom-up:
//!
//! - [`raw`] — low-level FFI mirroring `ladspa.h`.
//! - [`realtime`] — primitives safe to call from the host's realtime
//!   audio thread.
//! - [`port`] — port descriptors, hints, and runtime port access.
//! - [`Plugin`] — the user-facing trait. Implementations declare
//!   metadata, port shape, and lifecycle methods; the framework
//!   handles the C ABI.
//! - [`plugin_entry!`] — exposes a `Plugin` impl as the LADSPA
//!   `ladspa_descriptor` shared-object entry point.
//!
//! See `docs/decisions/` for the architectural decisions that
//! constrain this crate.

pub mod descriptor;
pub mod entry;
pub mod error;
mod macros;
pub mod plugin;
pub mod port;
pub mod raw;
pub mod realtime;

pub use error::InstantiateError;
pub use plugin::Plugin;
