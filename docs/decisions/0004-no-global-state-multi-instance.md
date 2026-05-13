# ADR 0004: Plugin instances are first-class; no global state

- Status: Accepted
- Date: 2026-05-13

## Context

LADSPA explicitly permits a host to call `instantiate()` multiple times
on the same `LADSPA_Descriptor`. Each call returns an independent
`LADSPA_Handle` representing a separate audio-processing instance with
its own state, buffers, and lifecycle. PipeWire, Ardour, and Audacity
all exercise this when the same plugin is placed on multiple tracks or
buses.

A framework can mishandle this in two ways:

1. **Hidden global state.** Storing plugin-instance data in a `static mut`
   or in a `OnceCell<Mutex<...>>`. Subsequent instantiations corrupt
   shared state.
2. **Implicit singletons.** Trait designs where the plugin is a
   zero-sized type and `&mut self` is faked from a `&'static` reference.

Rust's ownership system makes the correct shape natural: an
`instantiate` that returns `Self` by value gives each instance its own
storage. The challenge is to make this the *only* expressible shape.

## Decision

The `Plugin` trait is defined such that:

- `Plugin: Sized`.
- `fn instantiate(sample_rate: u32) -> Result<Self, InstantiateError>;`
  returns the instance by value.
- All mutable state lives in `Self`. Lifecycle methods
  (`activate`, `run`, `deactivate`) take `&mut self`.
- The framework's FFI shim allocates the instance on the host heap
  during `instantiate` (in the non-realtime instantiation phase),
  yields a raw `LADSPA_Handle` derived from the pointer, and reclaims
  it during `cleanup`.

The framework itself follows the same rule:

- No `static mut`.
- No `OnceCell` / `OnceLock` containing plugin or instance state.
- The only `static` items the framework emits are *immutable* metadata
  derived from `Plugin`'s associated consts: the `LADSPA_Descriptor`,
  port-name string tables, and port-descriptor arrays. These are read
  by the host and never mutated.

`Send` is not required on `Plugin`. Hosts call all lifecycle methods on
a single thread per instance (though different instances may live on
different threads). Requiring `Send` would foreclose plugin designs
that store `Rc` or thread-local handles internally.

## Consequences

Positive:

- Each instance gets its own state by construction. Misuse (a plugin
  author trying to share mutable state across instances) requires
  reaching for `unsafe`, a `Mutex<'static>`, or `lazy_static!` — all
  visible to code review.
- The framework's invariant ("no global state") is mechanically
  checkable: a `#[deny(static_mut_refs)]` lint plus a CI grep for
  `static mut` in `src/` is sufficient.
- Aligns with `CLAUDE.md` § "No global state" — this ADR formalises it.

Negative:

- Plugins that *want* shared resources (e.g. a single FFT plan reused
  across instances) must explicitly opt in via `Arc` or similar. The
  framework will not provide built-in shared state.

## Verification

- Integration test: load the same plugin via the framework's test
  harness, call `instantiate` twice, mutate one instance's state, and
  assert the other instance is unaffected.
- Static check: a `tests/` file containing `static mut FORBIDDEN: u32 =
  0;` is *not* added — instead CI greps `src/` for `static mut` and
  fails the build if any match is found.

## References

- `CLAUDE.md` § Development Principles ("No global state").
- `docs/architecture.md` § `Plugin`.
- Open question 4 in `docs/architecture.md`, now resolved by this ADR.
