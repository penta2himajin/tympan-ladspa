//! The [`RealtimeContext`] marker type.

use core::marker::PhantomData;

/// Compile-time witness that the current call stack is running on the
/// host's realtime audio thread.
///
/// LADSPA's `run()` callback executes on the host's realtime thread.
/// Any function reachable from there must be allocation-free,
/// lock-free, and free of blocking system calls (see `CLAUDE.md` §
/// Prohibitions).
///
/// `RealtimeContext` is a [zero-sized type] whose only purpose is to
/// be passed as an argument. A function that requires
/// `&RealtimeContext` is **declaring** at the type level that it is
/// safe to call from the realtime thread. Conversely, code that does
/// not have a `&RealtimeContext` in scope cannot call such functions
/// — the type system rules out the misuse at compile time.
///
/// Instances cannot be constructed by user code. The framework
/// fabricates one inside the FFI shim that adapts LADSPA's `run`
/// callback to the [`Plugin`] trait, and threads it through to the
/// user's [`Plugin::run`] implementation. Outside the crate there is
/// no public path to a `RealtimeContext`, so the only way user code
/// can call a realtime-only function is from within `run()`.
///
/// # Why a marker rather than `unsafe fn`?
///
/// Marking realtime-only functions as `unsafe fn` would not be
/// useful: they are memory-safe in the conventional Rust sense
/// (they don't dereference raw pointers, don't violate aliasing).
/// The hazard they create is *non-realtime behaviour on a realtime
/// thread* — a categorically different unsafety. A bespoke marker
/// lets the framework express that invariant separately from the
/// `unsafe` keyword's existing meaning.
///
/// # Send / Sync
///
/// `RealtimeContext` is [`Send`] but not [`Sync`]. Conceptually a
/// realtime context attaches to *one* thread (the audio thread).
/// `Send` permits passing it across `move` closure boundaries within
/// that thread; `!Sync` prevents sharing it across threads, which
/// would be nonsense — if the context were shared, the "I am on the
/// realtime thread" claim could not be true for every observer.
///
/// # Layout
///
/// `RealtimeContext` is a zero-sized type: `size_of::<RealtimeContext>()
/// == 0`. Taking a reference `&RealtimeContext` is the standard way
/// to pass it; this costs at most a pointer in the calling
/// convention but, with current `rustc`, the parameter is usually
/// optimised away entirely.
///
/// [zero-sized type]: https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts
/// [`Plugin`]: crate
/// [`Plugin::run`]: crate
///
/// # Compile-fail check
///
/// The type intentionally does not implement [`Sync`]; the following
/// doctest verifies that a future change removing the
/// `PhantomData<Cell<()>>` field would surface as a CI failure.
///
/// ```compile_fail
/// fn requires_sync<T: Sync>() {}
/// requires_sync::<tympan_ladspa::realtime::RealtimeContext>();
/// ```
pub struct RealtimeContext {
    /// Private field that closes the type to outside construction.
    /// Without this field, downstream crates could write
    /// `RealtimeContext {}` and forge a witness.
    _private: (),
    /// Opt out of [`Sync`]. The `Send` bound is automatic — a `*const
    /// ()` would also remove `Send`, which we want to keep — so we
    /// use [`PhantomData`] over a `Cell`-like type rather than over
    /// a raw pointer.
    _not_sync: PhantomData<core::cell::Cell<()>>,
}

impl RealtimeContext {
    /// Construct a `RealtimeContext`.
    ///
    /// Reachable only from within this crate. The framework's FFI
    /// shim that adapts LADSPA's `run` callback to the [`Plugin`]
    /// trait calls this exactly once per call into user code, then
    /// drops the value when the call returns.
    ///
    /// Callers are responsible for ensuring that the construction
    /// happens on the host's realtime thread. The function itself
    /// performs no thread-identity check — that would either be a
    /// system call (which the realtime path forbids) or a static
    /// `pthread_self` cache, neither of which is appropriate here.
    ///
    /// [`Plugin`]: crate
    pub(crate) fn new() -> Self {
        Self {
            _private: (),
            _not_sync: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn realtime_context_is_zero_sized() {
        assert_eq!(size_of::<RealtimeContext>(), 0);
    }

    #[test]
    fn realtime_context_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RealtimeContext>();
    }

    #[test]
    fn realtime_only_pattern_threads_through_reference() {
        // Demonstrates the type-level guard pattern: a function that
        // requires &RealtimeContext can only be called when one is
        // in scope. Inside `run()`, the framework provides such a
        // reference; elsewhere there is no public path to obtain one.
        fn realtime_only(_rt: &RealtimeContext) -> u32 {
            42
        }

        let rt = RealtimeContext::new();
        assert_eq!(realtime_only(&rt), 42);
    }
}
