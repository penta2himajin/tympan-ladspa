# Plugin Author Guide

This guide collects the non-obvious bits of writing a LADSPA plugin on
top of `tympan-ladspa` — the things scattered across module docstrings,
ADRs, and example READMEs, pulled into one place. For an end-to-end
working starting point use [`examples/gain/`](../examples/gain/) (the
smallest viable consumer of the framework); for a more realistic
shape see [`examples/noise-gate/`](../examples/noise-gate/) and
[`examples/delay/`](../examples/delay/).

## Project setup

Your plugin is a `cdylib` crate that depends on this framework. The
minimum `Cargo.toml`:

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[lib]
crate-type = ["cdylib"]

[dependencies]
tympan-ladspa = { git = "https://github.com/penta2himajin/tympan-ladspa" }

[profile.release]
panic = "abort"      # see § Panic strategy below
lto = "thin"         # smaller .so, marginal codegen win
strip = "symbols"    # smaller .so on disk
```

The crate has zero non-path dependencies of its own, so adding it
keeps your dependency tree small.

## Panic strategy

LADSPA hosts are C code. A Rust `panic!()` that unwinds across the
C ABI boundary — for example out of `Plugin::run` and through the
framework's `extern "C" fn` shim — is **undefined behaviour**, even
when the immediate caller is also Rust.

The framework does not wrap your code in [`catch_unwind`] because
the catch handler would add overhead to every `run()` call and the
runtime metadata for unwinding bloats the binary. Instead:

1. Set `panic = "abort"` in your release profile. A panic anywhere
   in the plugin becomes an immediate process abort — no unwinding,
   no UB across the ABI boundary, and the binary shrinks.
2. Avoid `panic!` in `run`. `assert!`, `unwrap`, integer overflow
   in debug builds, slice indexing out of bounds — all of these
   panic. Replace them with explicit bounds checks during plugin
   development, or use the [`LogSink`](../src/realtime/log.rs)
   pattern to emit a diagnostic event off-thread rather than
   crashing.
3. Failures in `Plugin::instantiate` are first-class: return
   `Err(InstantiateError::...)` and the framework reports NULL to
   the host without panicking.

[`catch_unwind`]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html

## Plugin identity (`UNIQUE_ID`)

`Plugin::UNIQUE_ID` is part of your plugin's stable ABI. Host
configurations reference plugins by `UniqueID`; changing it across
versions silently breaks every config that names your plugin.

Conventions:

- Use a high-entropy value in the 32-bit space.
- For plugins intended for public distribution, coordinate with the
  [LADSPA central registry](https://ladspa.org/) before settling on
  an ID.
- For private / development plugins, the framework's example crates
  use small arbitrary values (`12_345`, `23_456`, `34_567`) — fine
  for local use, **not** for distribution.
- See [ADR 0004](decisions/0004-no-global-state-multi-instance.md)
  for why the framework treats `UNIQUE_ID` as immutable.

## Symbol visibility

The framework's [`plugin_entry!`](../src/macros.rs) macro emits
`#[no_mangle] pub unsafe extern "C" fn ladspa_descriptor`. That is
the only symbol your host needs to see.

By default a Rust `cdylib` exports `extern "C"` items but not plain
Rust public items — the rlib's Rust symbols are not visible from
outside the `.so`. The CI workflow asserts this in the `examples
(LADSPA SDK)` job:

```sh
nm -D --defined-only target/release/libyour_plugin.so \
  | grep -E ' T ladspa_descriptor$'
```

If you have additional `extern "C"` items in your crate that you
do not want exported, restrict visibility globally with a `.cargo/
config.toml`:

```toml
[build]
rustflags = ["-Cdefault-symbol-visibility=hidden"]
```

Then explicitly export only what the host needs (the framework's
macro already takes care of `ladspa_descriptor`).

## Port conventions

Audio ports first, then control ports. Many hosts assume audio
inputs come before audio outputs and that channel layouts are
contiguous. The example plugins all follow this:

```rust
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
```

[`PortDefault`](../src/port.rs) enumerates the nine defaults LADSPA
can express: `Minimum`, `Low`, `Middle`, `High`, `Maximum`, `Zero`,
`One`, `Hundred`, `Hz440`. Any other initial value must be encoded
implicitly via `with_bounds` and documented elsewhere — LADSPA
itself cannot represent it (see
[ADR 0002](decisions/0002-ports-as-const-slice.md) for the rationale).

Inside `Plugin::run`:

- `ports.audio_input(i)` — `&[Data]`, shared borrow.
- `ports.audio_output(i)` — `&mut [Data]`, exclusive borrow.
- `ports.control_input(i)` — `Data` (Copy).
- `ports.audio_in_out(i, j)` — `(&[Data], &mut [Data])` in one call.
  Use this when you need to read an input and write an output
  concurrently; the single-port accessors cannot express that under
  Rust's borrow checker.

## Realtime path discipline

`Plugin::run` executes on the host's realtime audio thread. Code
reachable from `run` must not:

1. Allocate. Pre-allocate every buffer in `instantiate` or
   `activate`. The framework's
   [`examples/gain/tests/no_alloc.rs`](../examples/gain/tests/no_alloc.rs)
   demonstrates the integration test that pins this invariant for
   the gain plugin; the same pattern works for any plugin.
2. Take a `Mutex`. Use atomics, the framework's
   [SPSC ring buffer](../src/realtime/ring.rs), or per-instance
   state owned exclusively by the plugin.
3. Make blocking syscalls (`write`, `read`, `open`, etc.). No
   `println!`, no file I/O, no `std::process`. For diagnostics, use
   [`LogSink`](../src/realtime/log.rs) — its
   `log(event)` is a single atomic try-push.
4. Spawn or join threads. `instantiate` is fine; `run` is not.

The framework's CI exercises these invariants:

- **Tier 2** — `assert_no_alloc` integration test pinned for the
  gain example;
- **Tier 2** — AddressSanitizer detects pointer / lifetime UB;
- **Tier 3** — ThreadSanitizer detects data races (especially
  through the SPSC ring buffer used for cross-thread events).

Mirror the assert-no-alloc pattern in your own plugin to extend the
guarantee. The mechanism is ~50 lines of test scaffolding.

## Diagnostic events: `LogSink`

When you need to emit a diagnostic event from `run` — "input
buffer larger than expected", "control value clamped",
"unrecognised mode", etc. — push it into a
[`LogSink`](../src/realtime/log.rs).
A small enum is the standard event shape:

```rust
#[derive(Debug, Clone, Copy)]
enum MyLogEvent {
    FeedbackClamped { requested: f32 },
}
```

Construct the sink in `instantiate`; the framework drops it during
`cleanup` and the drainer thread joins. See
[`examples/delay/`](../examples/delay/) for a working integration.

## Installing for LADSPA hosts

Stable plugin paths Linux hosts search:

- `~/.ladspa/` — per-user.
- `~/.config/ladspa/` — per-user, less common.
- `/usr/local/lib/ladspa/` — system, locally installed.
- `/usr/lib/ladspa/` — system, distro-packaged.
- `$LADSPA_PATH` — host override; colon-separated.

After building:

```sh
mkdir -p ~/.ladspa
cp target/release/libmy_plugin.so ~/.ladspa/
```

For PipeWire's filter-chain module, see the recipe in each example's
README — the relevant block goes in
`~/.config/pipewire/filter-chain.conf.d/*.conf`.

For Ardour, Audacity, and any LADSPA-aware host, the plugin appears
in the host's plugin browser once the `.so` is in one of the paths
above.

## Common pitfalls

| Pitfall | Symptom | Fix |
|---|---|---|
| Missing `crate-type = ["cdylib"]` | `target/release/libfoo.so` not produced | Add `[lib] crate-type = ["cdylib"]` to `Cargo.toml`. |
| `println!` / `dbg!` in `run` | `assert_no_alloc` test fails, or audible glitches on slow hosts | Use `LogSink` instead. |
| Two `&mut` outputs simultaneously | Borrow checker error | Use `audio_in_out` for the in→out pattern; otherwise sequence the writes. |
| `with_default(0.5)` | Default silently ignored (LADSPA can't express 0.5) | Use a `PortDefault` variant (`Middle` between bounds, or one of the four literals). |
| Changing `UNIQUE_ID` between releases | Existing host configs stop finding your plugin | Pick the ID once and never change it; bump `LABEL` or `NAME` instead if you must signal a difference. |
| `panic!` from `run` without `panic = "abort"` | UB on some hosts, hang on others | Set `panic = "abort"` in `[profile.release]`. |
| Spawning threads in `run` | Audio dropouts, sanitizer reports | Spawn in `instantiate` (e.g. via `LogSink::new`); never from `run`. |

## Where to read further

- [`docs/overview.md`](overview.md) — project scope and goals.
- [`docs/architecture.md`](architecture.md) — internal module layout.
- [`docs/decisions/`](decisions/) — ADRs covering each significant
  framework decision.
- The example crates under [`examples/`](../examples/) — full working
  plugins for the patterns described here.
