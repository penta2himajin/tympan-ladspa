# tympan-ladspa

[日本語](./README.ja.md)

A Rust framework for writing LADSPA plugins.

`tympan-ladspa` provides Rust abstractions over the LADSPA (Linux Audio
Developer's Simple Plugin API) C interface, enabling Rust applications to
implement audio plugins suitable for hosts such as PipeWire's filter-chain,
Ardour, and other LADSPA-compatible processors.

## Status

**Intended functionality complete.** Every "In scope" item from
[`docs/overview.md`](docs/overview.md) is implemented, and every
non-speculative entry in
[ADR 0005](docs/decisions/0005-ci-verification-strategy.md) (the
tiered CI plan) is wired up:

### Framework

- Layer 1 — low-level LADSPA FFI ([`src/raw/`](src/raw/)).
- Layer 2 — realtime primitives ([`src/realtime/`](src/realtime/)):
  the [`RealtimeContext`](src/realtime/context.rs) type-level marker,
  a lock-free [SPSC ring buffer](src/realtime/ring.rs), and a
  [`LogSink`](src/realtime/log.rs) wrapper packaging the
  "log from realtime, drain off-thread" pattern.
- Layer 3 — user-facing API: [`Plugin`](src/plugin.rs) trait,
  [`Ports`](src/port.rs) with `PortDescriptor` builders, and the
  [`plugin_entry!`](src/macros.rs) declarative macro that exports a
  LADSPA `ladspa_descriptor` entry point.

### Examples

Three reference plugins under [`examples/`](examples/):

- [`gain/`](examples/gain/) — minimal linear gain; alloc-free
  invariant pinned in CI.
- [`noise-gate/`](examples/noise-gate/) — hysteresis gate;
  multi-control + per-instance state demonstration.
- [`delay/`](examples/delay/) — feedback delay; demonstrates
  `LogSink` end-to-end.

### CI

| Tier | Checks |
|---|---|
| 1 | `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo build/test` on `stable` and MSRV `1.75`, `nm -D` symbol audit |
| 2 | `ladspa-sdk` integration: `analyseplugin` (metadata round-trip) and `applyplugin` (full lifecycle + numerical check) for every example; `assert_no_alloc` integration test pinning `CLAUDE.md` Prohibition 1 on `Plugin::run`; AddressSanitizer on the workspace (nightly + `-Zbuild-std`) |
| 3 | ThreadSanitizer (nightly + `-Zbuild-std`) — validates the SPSC ring buffer's Acquire/Release ordering on the 50 000-item concurrent exchange; `cargo-deny` supply-chain hygiene (advisories + licences + bans + sources) |

The framework is usable: write `impl Plugin for MyPlugin` and
`tympan_ladspa::plugin_entry!(MyPlugin)` in a `cdylib` crate and the
resulting `.so` loads in any LADSPA host. See
[`examples/gain/`](examples/gain/) for the minimal recipe and
[`docs/plugin-author-guide.md`](docs/plugin-author-guide.md) for the
collected practical recipes (panic strategy, `UNIQUE_ID`,
symbol visibility, realtime debugging, common pitfalls).

### Future work

The two ADR 0005 Tier 3 items not yet on CI are documented as
deferred in the ADR itself: the `strace` syscall allow-list
(fragile in the presence of glibc / kernel updates) and the
`criterion` regression bench (no realtime-path baseline yet — the
current `run()` is a thin slice iterator). Both have re-evaluation
triggers spelled out in
[ADR 0005 § Trigger for revisiting](docs/decisions/0005-ci-verification-strategy.md).
A multi-plugin `cdylib` variant of `plugin_entry!` is sketched in
[`src/macros.rs`](src/macros.rs)'s docstring; not implemented
because no in-tree consumer currently needs it.

## Naming

*Tympan* — the tympanal organ of moths, a membrane-based ultrasound sensor
on the abdomen of pyralid and noctuid moths. Evolved to detect the
echolocation calls of bats. The name reflects the library's role: a thin
membrane between the host audio engine and user-space Rust code.

## Quickstart for plugin authors

Add a new `cdylib` crate that depends on this framework, declare your
plugin, and invoke the entry-point macro:

```toml
# Cargo.toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
tympan-ladspa = { git = "https://github.com/penta2himajin/tympan-ladspa" }
```

```rust
// src/lib.rs
use tympan_ladspa::{
    plugin_entry,
    port::{PortDefault, PortDescriptor, Ports},
    realtime::RealtimeContext,
    InstantiateError, Plugin,
};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    const UNIQUE_ID: u32 = 0x_1234_5678;       // get yours from ladspa.org
    const LABEL: &'static str = "my_plugin";
    const NAME: &'static str = "My Plugin";
    const MAKER: &'static str = "your name";
    const COPYRIGHT: &'static str = "MIT OR Apache-2.0";

    fn ports() -> &'static [PortDescriptor] {
        static PORTS: &[PortDescriptor] = &[
            PortDescriptor::audio_input("In"),
            PortDescriptor::audio_output("Out"),
            PortDescriptor::control_input("Gain")
                .with_default(PortDefault::One)
                .with_bounds(0.0, 4.0),
        ];
        PORTS
    }

    fn instantiate(_sample_rate: u32) -> Result<Self, InstantiateError> {
        Ok(Self)
    }

    fn run(&mut self, _rt: &RealtimeContext, _frames: usize, ports: &mut Ports<'_>) {
        let gain = ports.control_input(2);
        let (input, output) = ports.audio_in_out(0, 1);
        for (i, o) in input.iter().zip(output.iter_mut()) {
            *o = *i * gain;
        }
    }
}

plugin_entry!(MyPlugin);
```

Build with `cargo build --release` and copy
`target/release/libmy_plugin.so` to `~/.ladspa/`. See
[`examples/gain/`](examples/gain/) for the same shape as a complete
crate.

## Development

The project's CI runs `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test`, and an `examples` job
that drives every example plugin end-to-end through the LADSPA SDK
([`.github/workflows/ci.yml`](.github/workflows/ci.yml);
[ADR 0005](docs/decisions/0005-ci-verification-strategy.md) records
the tiered verification strategy).

To run the same fmt and clippy checks locally before every `git push`,
opt into the repository's pre-push hook:

```sh
git config core.hooksPath .githooks
```

The hook lives in [`.githooks/pre-push`](.githooks/pre-push). It is a
no-op when no `*.rs`, `Cargo.toml`, or `Cargo.lock` files changed in
the pushed range, so documentation-only pushes are not slowed down.
Bypass it for a single push with `git push --no-verify`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

## Documentation

| Doc | Content |
|---|---|
| [`docs/overview.md`](docs/overview.md) | Project purpose, scope, comparison to existing implementations |
| [`docs/architecture.md`](docs/architecture.md) | API design and module layout |
| [`docs/plugin-author-guide.md`](docs/plugin-author-guide.md) | Practical recipes for writing a plugin on top of the framework |
| [`docs/references.md`](docs/references.md) | LADSPA spec, PipeWire integration, prior art |
| [`docs/decisions/`](docs/decisions/) | Architectural Decision Records |
| [`docs/handoff-protocol.md`](docs/handoff-protocol.md) | Session handoff protocol for long-running work |

## Examples

| Example | Description |
|---|---|
| [`examples/gain/`](examples/gain/) | Minimal linear-gain plugin. Smallest viable consumer of the framework. Pinned alloc-free in CI. |
| [`examples/noise-gate/`](examples/noise-gate/) | Hysteresis noise gate. Demonstrates multi-control input, per-instance state, and DSP logic factored into a pure function for unit testing. |
| [`examples/delay/`](examples/delay/) | Feedback delay line. Pre-allocates a `Vec<f32>` ring in `instantiate`, resets state in `activate`, exercises three control ports and the `audio_in_out` escape hatch. |
