# Overview

## Purpose

`tympan-ladspa` is a Rust framework for implementing **LADSPA plugins**
— the Linux audio plugin format used by hosts including PipeWire's
filter-chain, Ardour, Audacity, and many JACK-based applications.

The goal is to enable Rust applications to:

- Implement audio processing plugins (effects, generators, analysers)
- Build noise-suppression, voice-effect, or filter plugins for use with
  PipeWire filter-chain
- Distribute drop-in `.so` files that work in any LADSPA host

… without writing C.

## Why this exists

LADSPA is defined in a single C header, `ladspa.h`. It is intentionally
small: roughly 10 callbacks and a descriptor struct. But writing a
LADSPA plugin in Rust still requires manual FFI, careful lifetime
management around host-owned buffer pointers, and discipline around
realtime constraints.

Existing options for LADSPA development from Rust are:

| Approach | Status | Trade-off |
|---|---|---|
| Hand-rolled FFI from Rust | Each user reinvents the wheel | Hundreds of lines of `unsafe` per plugin |
| C with `bindgen` | Possible | Still requires writing the LADSPA descriptor and dispatcher in C-style Rust |
| `ladspa` crate (older, archived) | Pre-2020, no longer maintained | Stale; no PipeWire-era patterns |

This framework fills the Rust gap with a modern, maintained alternative
that takes realtime correctness seriously.

## Scope

### In scope

- LADSPA descriptor construction (UniqueID, Label, Properties, port
  metadata)
- Plugin lifecycle callbacks (`instantiate`, `connect_port`, `activate`,
  `run`, `deactivate`, `cleanup`)
- Port definitions: audio/control × input/output
- Port range hints (default values, bounds, log/integer/sample-rate
  scaling)
- Realtime-safe primitives (lock-free ring buffers, atomic state
  helpers)
- `cdylib` packaging that exposes `ladspa_descriptor` as the C entry
  point
- Example plugins: minimal gain, simple noise gate, ring-buffered delay

### Out of scope

- LV2 (separate, more complex format; possibly a future sibling crate)
- VST3 / AU / AAX (commercial DAW formats; different APIs)
- Signal-processing algorithms (DSP, ML) — these belong in consumer
  crates that depend on `tympan-ladspa`
- Plugin GUIs (LADSPA has no GUI spec; LV2 or external mechanisms are
  required)
- `run_adding` API (an optional LADSPA extension; not commonly used
  by modern hosts)

## Naming

*Tympan* refers to the tympanal organ — a membrane-based hearing organ on
the abdomen of moths in families such as Pyralidae and Noctuidae. The
organ evolved as a defence against bat echolocation: it captures
ultrasound and converts vibration into neural signals via attached
chordotonal receptors.

The analogy:

- A tympanal organ sits between the outside world and the moth's nervous
  system, converting one physical domain (air pressure) into another
  (nerve impulses).
- `tympan-ladspa` sits between the host audio engine and user-space
  Rust code, converting one programming domain (C ABI, raw pointers,
  realtime callbacks) into another (safe Rust types, ownership,
  lifetimes).

The second word `ladspa` is the plugin format this crate targets:
Linux Audio Developer's Simple Plugin API.

## Status

**Design phase.** As of the initial commit:

- No source code in `src/`
- API design documented in [`architecture.md`](architecture.md)
- Reference material gathered in [`references.md`](references.md)

Implementation will begin once the API design is reviewed and
stabilised.

## Target audience

- Rust developers building audio plugins for Linux hosts
- PipeWire filter-chain users who want custom DSP without writing C
- Researchers prototyping audio processing pipelines that need to
  integrate at the LADSPA layer
- Any Linux user replacing system-wide audio enhancement (denoising,
  EQ, gating) with a plugin they can audit

Not intended for:

- Application-level audio playback (use `cpal`, `rodio`, or PipeWire
  client APIs)
- Cross-platform plugin formats (LADSPA is Linux-centric; LV2 has
  broader portability)
- DAW-specific plugin formats (VST3, AU) — those use entirely
  different APIs

## Comparison to alternatives

### vs. LV2 (and the `lv2` crate)

LV2 is the successor to LADSPA: richer, more complex, with support for
GUIs, message passing, atoms, and arbitrary extensions. The `lv2`
Rust crate exists and is maintained.

LADSPA's simplicity is a feature, not a limitation. PipeWire's
`filter-chain` module supports both, but LADSPA configurations are
shorter and less error-prone. For pure audio effects without GUIs or
dynamic configuration, LADSPA remains the lower-friction choice.

`tympan-ladspa` and `lv2` could conceivably share a Rust developer
base; the choice between them depends on whether the plugin needs LV2's
additional features.

### vs. the older `ladspa` crate

A `ladspa` crate exists on crates.io, last updated several years ago.
It predates the PipeWire era and was designed with JACK and standalone
hosts in mind. Its realtime-safety story is informal. `tympan-ladspa`
aims to provide:

- Explicit realtime-context type-level guarantees
- Better fit with PipeWire filter-chain configuration patterns
- Active maintenance and modern Rust idioms (const generics, trait
  objects, builder APIs)

### vs. hand-rolled FFI

Any Rust developer can implement a LADSPA plugin directly using
`bindgen` or by typing out `ladspa.h` declarations. The result is
hundreds of lines of `unsafe` and easy-to-miss memory-safety pitfalls
around host-owned pointers. `tympan-ladspa` centralises that
boilerplate.

## Relationship to PipeWire

LADSPA plugins compiled with `tympan-ladspa` are loaded by PipeWire
via the `libpipewire-module-filter-chain` module. A typical PipeWire
configuration:

```
context.modules = [
  { name = libpipewire-module-filter-chain
    args = {
      node.description = "Example filter"
      filter.graph = {
        nodes = [
          {
            type = ladspa
            name = my-filter
            plugin = my_plugin       # path to .so without extension
            label = my_filter_label  # the LADSPA Label
          }
        ]
      }
      capture.props = { node.name = "input.example" node.passive = true }
      playback.props = { node.name = "example_source" media.class = Audio/Source }
    }
  }
]
```

The framework's example plugins target this exact deployment shape.
