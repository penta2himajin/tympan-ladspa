# tympan-ladspa

[日本語](./README.ja.md)

A Rust framework for writing LADSPA plugins.

`tympan-ladspa` provides Rust abstractions over the LADSPA (Linux Audio
Developer's Simple Plugin API) C interface, enabling Rust applications to
implement audio plugins suitable for hosts such as PipeWire's filter-chain,
Ardour, and other LADSPA-compatible processors.

## Status

**Initial scope complete.** Every "In scope" item from
[`docs/overview.md`](docs/overview.md) is in tree:

- Layer 1 — low-level LADSPA FFI ([`src/raw/`](src/raw/)).
- Layer 2 — realtime primitives ([`src/realtime/`](src/realtime/)):
  the [`RealtimeContext`](src/realtime/context.rs) type-level marker
  and a lock-free [SPSC ring buffer](src/realtime/ring.rs).
- Layer 3 — the user-facing API: [`Plugin`](src/plugin.rs) trait,
  [`Ports`](src/port.rs) with `PortDescriptor` builders, and the
  [`plugin_entry!`](src/macros.rs) declarative macro that exports a
  LADSPA `ladspa_descriptor` entry point.
- Three reference plugins under [`examples/`](examples/): a gain, a
  hysteresis noise gate, and a feedback delay line.
- CI Tier 1 + Tier 2 from
  [ADR 0005](docs/decisions/0005-ci-verification-strategy.md): every
  PR runs fmt, clippy, build, test (MSRV 1.75 and stable), and
  drives all three example plugins through `analyseplugin` and
  `applyplugin` end-to-end. An `assert_no_alloc`-style integration
  test pins `CLAUDE.md` Prohibition 1 (no heap allocation in
  `Plugin::run`) for the gain example.

The framework is usable: write `impl Plugin for MyPlugin` and
`tympan_ladspa::plugin_entry!(MyPlugin)` in a `cdylib` crate and the
resulting `.so` loads in any LADSPA host. See
[`examples/gain/`](examples/gain/) for the minimal recipe.

Future work (not yet started): higher CI tiers from ADR 0005 (ASAN,
TSAN, syscall allow-list, `criterion` benches),
documentation site, and additional reference plugins as needs
appear.

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
