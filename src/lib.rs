//! A safe Rust framework for writing LADSPA plugins.
//!
//! Implementation is in progress. The public API surface described in
//! [`docs/architecture.md`](https://github.com/penta2himajin/tympan-ladspa/blob/main/docs/architecture.md)
//! has not yet been built; the only module currently populated is
//! [`raw`], the low-level FFI layer.
//!
//! See `docs/decisions/` for the architectural decisions that constrain
//! this crate.

pub mod raw;
