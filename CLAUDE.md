# tympan-ladspa

## Overview

Rust framework for writing LADSPA plugins. The library provides safe
abstractions over the LADSPA C interface so that Rust authors can
implement audio effects suitable for PipeWire filter-chain, Ardour, and
other LADSPA-compatible hosts without using C.

Detailed design lives under @docs/overview.md and @docs/architecture.md.

## Project Structure

Currently in design phase. No source code yet.

```
docs/                    # Design and references
.github/                 # Issue/PR templates
```

Once implementation begins, the layout will follow:

```
src/                     # Public API (high-level, safe)
src/raw/                 # Low-level FFI: ladspa.h types and descriptor
src/realtime/            # Realtime-safe primitives (lock-free, alloc-free)
examples/                # Reference plugins (e.g. gain, noise gate)
tests/                   # Integration tests
```

## Development Setup

Required toolchain:

- Rust 1.75+
- A C compiler (for any test harnesses that load plugins via the host
  LADSPA loader)

Target hosts for verification:

- PipeWire 0.3.40+ via `libpipewire-module-filter-chain`
- Ardour, Audacity, or any LADSPA-aware host

## Build & Test

Once implementation starts:

```bash
cargo build --release
cargo test
```

The build produces a `cdylib` (`.so` on Linux). To load a plugin into
PipeWire's filter-chain:

```bash
mkdir -p ~/.ladspa
cp target/release/lib<plugin-name>.so ~/.ladspa/
# Then reference the plugin in ~/.config/pipewire/filter-chain.conf.d/
```

Verification: `listplugins` (from the `ladspa-sdk` package) should
enumerate the plugin's UniqueID, label, and ports.

## Development Principles

- **Realtime safety is non-negotiable.** The `run()` callback executes on
  the host's realtime audio thread. Code in this path must be
  allocation-free, lock-free, and free of system calls. Use the `realtime`
  module primitives.
- **Stable plugin identity.** LADSPA plugins are identified by a globally
  unique 32-bit `UniqueID`. The framework enforces that plugin authors
  declare a UniqueID at compile time and never change it across versions
  of the same plugin.
- **Match LADSPA semantics, not LADSPA naming.** Port descriptors and hint
  bitfields retain original semantics, but APIs use Rust-natural names
  (`PortKind::AudioInput` not `LADSPA_PORT_AUDIO | LADSPA_PORT_INPUT`).
- **No global state.** Plugin instances are first-class objects; the
  framework never relies on `static mut` or singletons.

## Architectural Boundaries

- `raw` module is the only place that contains the LADSPA C ABI
  declarations.
- `realtime` module never allocates and never returns `Result` values
  containing `String` or other heap types.
- Public API surface lives in `lib.rs` and re-exports from internal
  modules.
- `examples/` plugins must compile to `cdylib` and load in a real LADSPA
  host. Non-cdylib examples belong in `tests/` or as doc-tests.

## Prohibitions

1. Do not allocate memory in any function called from `run()` or its
   transitive callees. Pre-allocate buffers in `instantiate()` or
   `activate()`.
2. Do not call `std::sync::Mutex::lock()` from realtime code paths. Use
   lock-free primitives (`crossbeam`, atomics) instead.
3. Do not introduce dependencies on async runtimes (`tokio`, `async-std`).
   This is a sync, realtime-oriented library.
4. Do not depend on external C libraries beyond LADSPA's own `ladspa.h`.
5. Do not expose `unsafe fn` in the public API without a clearly documented
   safety contract. Internal `unsafe` is encapsulated behind safe wrappers.
6. Do not call any function that might block or wait on I/O from
   realtime code (no `println!`, no file I/O, no allocator calls).

## Git Conventions

- Scoped Conventional Commits: `feat(raw):`, `fix(realtime):`,
  `docs(arch):`.
- Scopes follow the module structure: `raw`, `realtime`, `api`,
  `examples`, `docs`, `meta` (CI, README, license).
- Breaking changes use `!` notation and require a corresponding entry
  in `docs/decisions/` (when that directory exists).
- PRs link a handoff issue with `Closes #N` or `Refs #N`.

## Session Handoff

Long-running workstreams use GitHub issues for cross-session continuity.
See @docs/handoff-protocol.md for the full protocol.

- Label: `session-handoff`
- One issue per workstream (not per session)
- On session start, read the relevant handoff issue and confirm the
  **Next action** with the user before executing.
