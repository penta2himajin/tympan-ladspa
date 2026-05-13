# ADR 0001: Skip `run_adding` / `set_run_adding_gain`

- Status: Accepted
- Date: 2026-05-13

## Context

`ladspa.h` declares two optional callbacks alongside the mandatory `run`:

```c
void (*run_adding)(LADSPA_Handle, unsigned long SampleCount);
void (*set_run_adding_gain)(LADSPA_Handle, LADSPA_Data Gain);
```

`run_adding` accumulates output into the host's buffers (`*out += result *
gain`) instead of overwriting (`*out = result`). It was introduced to let
hosts mix multiple plugin outputs onto a shared bus without an explicit
intermediate buffer and add loop.

Survey of current ecosystem:

- **PipeWire `module-filter-chain`** (the primary deployment target for
  this framework): calls only `run`. `run_adding` is never invoked.
- **Ardour**: legacy code paths reference `run_adding`; modern signal
  routing does not exercise it.
- **LADSPA SDK `analyseplugin`**: reports whether `run_adding` is present
  but does not exercise it.
- **LV2**: dropped the concept entirely. The successor format expects
  hosts to manage mix-bus accumulation themselves.

Hosts that encounter a plugin with a NULL `run_adding` pointer fall back
to calling `run` and performing accumulation themselves. There is no
correctness penalty for omitting it.

## Decision

`tympan-ladspa` does not expose `run_adding` or `set_run_adding_gain`.

- The `LADSPA_Descriptor` fields for both function pointers are emitted
  as `NULL`.
- The `Plugin` trait has no corresponding method.
- Documentation will note this explicitly so users searching for the API
  understand it is an intentional omission, not an oversight.

## Consequences

Positive:

- The realtime path stays a single `run` function. No per-instance
  atomic for the run-adding gain. No framework-side mixdown loop.
- The `Plugin` trait stays small.
- FFI dispatch is one function pointer, not three.

Negative:

- Hosts that do invoke `run_adding` will fall back to `run` + their own
  accumulation. This is the documented LADSPA behaviour, but on those
  hosts the plugin loses the (theoretical) optimisation of avoiding a
  temporary buffer.

## Reversal path

Adding `run_adding` later does not break the public API:

- A new trait method `fn run_adding(&mut self, rt: &RealtimeContext,
  frames: usize, ports: &mut Ports, gain: f32)` can be introduced with a
  default implementation that performs `run` followed by accumulation.
- The descriptor field gets populated only when the method is overridden
  (detected via a separate marker or a const associated bool), preserving
  zero-overhead for plugins that opt out.

This reversal will be re-evaluated if a user reports a concrete deployment
where `run_adding` materially affects performance.

## References

- `ladspa.h`, fields `run_adding` and `set_run_adding_gain` on
  `LADSPA_Descriptor`.
- `docs/overview.md` § Out of scope — this ADR formalises that bullet.
