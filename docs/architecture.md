# Architecture

This document describes the planned architecture. Implementation has not
begun. The pre-implementation design questions have been resolved; see
[`docs/decisions/`](decisions/README.md). Further changes that affect
public API or invariants should be recorded as new ADRs there.

## Module layout

```
tympan-ladspa/
├── src/
│   ├── lib.rs            # Re-exports; public API surface
│   ├── plugin.rs         # Plugin trait, lifecycle
│   ├── descriptor.rs     # LADSPA_Descriptor construction
│   ├── port.rs           # Port definitions, hints, connections
│   ├── entry.rs          # ladspa_descriptor entry-point macro
│   ├── raw/              # Low-level: ladspa.h FFI declarations
│   │   ├── mod.rs
│   │   ├── types.rs      # LADSPA_Data, LADSPA_PortDescriptor, etc.
│   │   └── descriptor.rs # struct LADSPA_Descriptor as_repr_C
│   └── realtime/         # Realtime-safe primitives
│       ├── mod.rs
│       ├── context.rs    # RealtimeContext marker type
│       ├── ring.rs       # Lock-free SPSC ring buffer
│       └── state.rs      # Atomic state machine helpers
├── examples/
│   ├── gain/             # Trivial linear gain plugin
│   └── noise-gate/       # Simple amplitude-threshold gate
└── tests/
    └── ...               # Integration tests using ladspa-sdk's analyseplugin
```

## Layer model

Three conceptual layers, isolated by module boundary:

### Layer 1: `raw` — unsafe FFI

- Sole owner of `#[repr(C)]` LADSPA type declarations
- Sole consumer of raw `extern "C"` function pointer types
- Provides direct mappings of `LADSPA_Data` (= `f32`),
  `LADSPA_PortDescriptor` (bitfield u32), etc.

Users of `tympan-ladspa` should not need to touch this module. It
exists for the framework's internal use and for advanced users who
need to bypass the higher-level abstractions.

### Layer 2: `realtime` — zero-allocation primitives

- No allocator usage
- No `std::sync::Mutex`, no `std::collections::HashMap`
- Lock-free SPSC ring buffers (built on `crossbeam-utils`)
- Atomic state machines for plugin lifecycle
- A `RealtimeContext` zero-sized marker that:
  - Is required as a parameter for any function safe to call from `run()`
  - Cannot be constructed outside the framework
  - Acts as a compile-time witness of realtime safety

This layer's invariant: any function reachable from `run()` must accept
`&RealtimeContext` and contain no heap operations.

### Layer 3: Public API — safe, idiomatic

- `Plugin` trait
- `Port`, `PortKind`, `PortHints` builders
- Lifetime-bounded references to host-provided buffers
- Result types for fallible operations during `instantiate`

This is the layer 95% of users will interact with.

## Core abstractions

### `Plugin`

The top-level trait implemented by consumers. Maps to LADSPA's plugin
instance lifecycle.

```text
trait Plugin: Sized {
    const UNIQUE_ID: u32;
    const LABEL: &'static str;
    const NAME: &'static str;
    const MAKER: &'static str;
    const COPYRIGHT: &'static str;

    fn ports() -> &'static [PortDescriptor];

    fn instantiate(sample_rate: u32) -> Result<Self, InstantiateError>;
    fn activate(&mut self) {}
    fn run(&mut self, rt: &RealtimeContext, frames: usize, ports: &mut Ports);
    fn deactivate(&mut self) {}
}
```

The framework provides the LADSPA entry point as a macro:

```text
tympan_ladspa::plugin_entry!(MyPlugin);
```

This expands to the `#[no_mangle] extern "C" fn ladspa_descriptor` that
hosts query when loading the `.so`.

### `PortDescriptor` and `Ports`

Port metadata is declared at compile time:

```text
static PORTS: &[PortDescriptor] = &[
    PortDescriptor::audio_input("In"),
    PortDescriptor::audio_output("Out"),
    PortDescriptor::control_input("Gain")
        .default(1.0)
        .bounds(0.0, 4.0),
];
```

During `run()`, the framework presents connected ports as a typed
`Ports` struct:

```text
fn run(&mut self, rt: &RealtimeContext, frames: usize, ports: &mut Ports) {
    let input  = ports.audio_input(0);   // &[f32]
    let output = ports.audio_output(1);  // &mut [f32]
    let gain   = ports.control_input(2); // f32 (single value)

    for (out, &i) in output.iter_mut().zip(input) {
        *out = i * gain;
    }
}
```

### `RealtimeContext`

Identical purpose to its counterpart in sibling tympan crates: a
zero-sized marker that compile-checks realtime safety. Instances are
passed by reference from the framework's `run` harness to user code.
They have no fields and no way to be constructed from user code.

## Cross-cutting concerns

### Plugin identity

LADSPA uses a 32-bit `UniqueID`. Authors should obtain one from the
[LADSPA central registry](https://ladspa.org/) or generate a high
entropy value. The framework enforces (via a const assertion) that the
UniqueID is non-zero, and warns about values in the reserved low range.

The `UniqueID` is part of the plugin's stable ABI. The framework does
not allow it to change across versions of the same plugin; doing so
would break configurations that reference the plugin by ID.

### `cdylib` packaging

Users configure their `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]
```

The framework provides build-script helpers to set the correct `soname`
and symbol visibility flags (e.g. `-Cdefault-symbol-visibility=hidden`
for everything except `ladspa_descriptor`).

### Realtime logging

Realtime code cannot log via `tracing` or `log` (both allocate).
The `realtime` module provides a lock-free log queue for capturing
diagnostic events from `run()`. A separate non-realtime thread (if the
host permits one to be spawned during `instantiate`) drains the queue.

## Resolved design questions

Four pre-implementation questions have been settled. Each is recorded as
an ADR under [`docs/decisions/`](decisions/README.md):

- **`run_adding` / `set_run_adding_gain`** — skipped entirely.
  See [ADR 0001](decisions/0001-skip-run-adding.md).
- **Port declaration shape** — `&'static [PortDescriptor]`, not const
  generics. See [ADR 0002](decisions/0002-ports-as-const-slice.md).
- **`#[derive(Plugin)]`** — not provided in the initial release; plugins
  are written as hand-rolled trait impls. See
  [ADR 0003](decisions/0003-trait-only-no-derive-macro.md).
- **Multiple plugin instances** — handled by `Plugin: Sized` returning
  `Self` from `instantiate`, with a framework-wide ban on global state.
  See [ADR 0004](decisions/0004-no-global-state-multi-instance.md).
