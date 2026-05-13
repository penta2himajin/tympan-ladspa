//! Pin the realtime no-allocation invariant for `Plugin::run`.
//!
//! `CLAUDE.md` § Prohibitions item 1 forbids heap allocation in code
//! reachable from the realtime audio thread. This integration test
//! replaces the binary's global allocator with one that records
//! allocations made while a thread-local guard is set, then drives the
//! gain plugin's `run` callback through the framework's FFI shim
//! many times inside the guarded region. A single allocation fails
//! the test.
//!
//! The guard is thread-local, so other tests sharing the binary (none
//! currently) are unaffected. Allocations made during test setup —
//! constructing the `DescriptorBundle`, instantiating, connecting
//! ports, activating — happen outside the guard and are allowed.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::c_ulong;
use std::sync::atomic::{AtomicUsize, Ordering};

use tympan_gain::Gain;
use tympan_ladspa::descriptor::{Callbacks, DescriptorBundle};
use tympan_ladspa::entry::{
    activate_shim, cleanup_shim, connect_port_shim, deactivate_shim, instantiate_shim, run_shim,
};
use tympan_ladspa::raw;

thread_local! {
    /// Per-thread flag. When `true`, every `alloc` call increments
    /// [`VIOLATIONS`]. `const { ... }` initializer means accessing the
    /// slot does not itself allocate the first time on a new thread.
    static GUARD: Cell<bool> = const { Cell::new(false) };
}

/// Process-wide counter of allocations observed while at least one
/// thread had its guard active. The test reads this before and after
/// the guarded region and asserts the difference is zero.
static VIOLATIONS: AtomicUsize = AtomicUsize::new(0);

struct AssertNoAlloc;

// SAFETY: Forwarding `alloc` and `dealloc` to the system allocator
// is sound by definition. The bookkeeping branch reads a `Cell<bool>`
// in TLS and increments an atomic — neither operation can recurse
// into the allocator.
unsafe impl GlobalAlloc for AssertNoAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if GUARD.with(|c| c.get()) {
            VIOLATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: contract upheld by GlobalAlloc.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: contract upheld by GlobalAlloc.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: AssertNoAlloc = AssertNoAlloc;

fn callbacks() -> Callbacks {
    Callbacks {
        instantiate: instantiate_shim::<Gain>,
        connect_port: connect_port_shim::<Gain>,
        run: run_shim::<Gain>,
        cleanup: cleanup_shim::<Gain>,
        activate: Some(activate_shim::<Gain>),
        deactivate: Some(deactivate_shim::<Gain>),
    }
}

fn enter_guard() {
    GUARD.with(|c| c.set(true));
}

fn leave_guard() {
    GUARD.with(|c| c.set(false));
}

#[test]
fn gain_run_does_not_allocate() {
    let bundle = DescriptorBundle::<Gain>::build(callbacks());
    // SAFETY: `bundle` owns the descriptor; the pointer is valid for
    // the bundle's lifetime, which spans the rest of this test.
    let d = unsafe { &*bundle.descriptor_ptr() };

    // Pre-touch the TLS slot and the atomic from outside the guard
    // so any first-time-on-this-thread bookkeeping the runtime does
    // doesn't show up in the counter.
    GUARD.with(|c| c.get());
    let _ = VIOLATIONS.load(Ordering::Relaxed);

    // SAFETY: the call below dereferences a descriptor we just
    // published. All subsequent unsafe calls in this test follow the
    // standard LADSPA host call pattern documented on each shim.
    let handle = unsafe { (d.instantiate.unwrap())(d as *const _, 48_000) };
    assert!(!handle.is_null(), "instantiate returned NULL");

    // Allocate test buffers *before* entering the guard. The guard
    // only checks that `run` itself is alloc-free, not the setup.
    let frame_sizes = [1usize, 8, 64, 256, 1024];
    let max_frames = *frame_sizes.iter().max().unwrap();
    let mut input: Vec<raw::Data> = (0..max_frames)
        .map(|i| ((i as f32) * 0.01).sin() * 0.5)
        .collect();
    let mut output: Vec<raw::Data> = vec![0.0; max_frames];
    let mut gain: raw::Data = 1.5;

    // SAFETY: `handle` is the live instance just returned above.
    // The buffers we pass into `connect_port` live in this stack
    // frame for the rest of the test.
    unsafe {
        (d.connect_port.unwrap())(handle, 0, input.as_mut_ptr());
        (d.connect_port.unwrap())(handle, 1, output.as_mut_ptr());
        (d.connect_port.unwrap())(handle, 2, &mut gain);
        (d.activate.unwrap())(handle);
    }

    let baseline = VIOLATIONS.load(Ordering::Relaxed);
    enter_guard();

    // Drive `run` many times at varied frame counts. Each call
    // walks the FFI shim path through `Plugin::run` for `Gain`.
    for &frames in &frame_sizes {
        for _ in 0..32 {
            // SAFETY: `handle` is the live instance from above; ports
            // are all bound; `frames` does not exceed buffer length.
            unsafe { (d.run.unwrap())(handle, frames as c_ulong) };
        }
    }

    leave_guard();
    let observed = VIOLATIONS.load(Ordering::Relaxed) - baseline;

    // SAFETY: balanced lifecycle teardown.
    unsafe {
        (d.deactivate.unwrap())(handle);
        (d.cleanup.unwrap())(handle);
    }

    // Sanity check: a non-zero gain * non-zero input must produce
    // non-zero output. Catches the trivial case where the test runs
    // `run` 0 times due to an empty frame_sizes table.
    assert!(
        output.iter().any(|&v| v != 0.0),
        "output is all zero; the run loop may not have executed",
    );

    assert_eq!(
        observed, 0,
        "Plugin::run() allocated {observed} times during the guarded region",
    );
}
