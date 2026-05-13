# ADR 0002: Declare ports as `&'static [PortDescriptor]`

- Status: Accepted
- Date: 2026-05-13

## Context

Port metadata (audio/control × input/output, name, range hints) must be
declared at compile time so that the `LADSPA_Descriptor` published from
`ladspa_descriptor()` can reference static arrays of `PortDescriptor`,
`PortRangeHint`, and name strings.

Two shapes were considered:

1. **Const generic over port count**

   ```text
   trait Plugin<const N: usize>: Sized {
       const PORTS: [PortDescriptor; N];
       ...
   }
   ```

   The port count becomes part of the type. `Ports<N>` accessors could
   in principle use `[_; N]` instead of slices, opening the door to
   bounds-check elision and stack-resident port tables.

2. **Const slice**

   ```text
   trait Plugin: Sized {
       fn ports() -> &'static [PortDescriptor];
       ...
   }
   ```

   The port count is a runtime value (known at compile time but not in
   the type system). `Ports` accessors take `usize` indices and slice
   into runtime arrays.

## Decision

Use the const slice shape. The `Plugin` trait declares:

```text
fn ports() -> &'static [PortDescriptor];
```

and the framework's `Ports` struct exposes accessors keyed by port index.

## Consequences

Positive:

- The `Plugin` trait stays free of generic parameters. This keeps the
  `plugin_entry!` macro, the FFI dispatch in `src/entry.rs`, and every
  example plugin's signature simple.
- Composing ports from helper functions (`fn standard_in_out_ports() ->
  &'static [PortDescriptor]`) is trivial. With const generics, the same
  composition requires either macro tricks or unstable features.
- Adding or removing a port in user code does not propagate type changes
  through the rest of the plugin's source.

Negative:

- Port accesses (`ports.audio_input(0)`) carry a bounds check. For
  LADSPA's typical port counts (well under 16) this is negligible.
- The port count is not enforceable at the type level. The framework
  must validate index/port-kind consistency at the boundary between
  user code and the host (during `connect_port` and at the entry to
  `run`).

## Reversal path

Migrating to const generics later would be a breaking change to the
`Plugin` trait. It is unlikely to be motivated by performance (the
bounds-check cost is dwarfed by audio I/O) but could be revisited if
const generics gain features that make port composition ergonomic
(e.g. stable concat of const arrays).

## References

- `docs/architecture.md` § `PortDescriptor` and `Ports`.
- Open question 2 in `docs/architecture.md`, now resolved by this ADR.
