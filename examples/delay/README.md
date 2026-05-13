# tympan-delay

A feedback delay line built on
[`tympan-ladspa`](../../). Reads samples written some number of
samples ago, mixes them into the output, and re-injects a feedback
proportion into the buffer.

This crate exists primarily as a worked example that:

- Pre-allocates a `Vec<f32>` ring in `Plugin::instantiate` and uses
  it for the entire plugin lifetime — no allocation in the realtime
  path.
- Uses `activate()` as the spec-blessed place to reset state (clears
  the buffer when the host re-activates the plugin).
- Shows the framework's `Ports::audio_in_out` escape hatch on a more
  realistic DSP pattern than the gain or noise-gate examples.
- Factors per-sample processing into a pure `apply_delay` function
  for unit testing without LADSPA machinery.

## Why not `tympan_ladspa::realtime::ring::SpscRing`?

The framework's [SPSC ring buffer](../../src/realtime/ring.rs) is a
**FIFO queue** with push/pop semantics and no random access. A delay
line is a **circular buffer with random-access read** at "N samples
ago" and overwrite-on-every-sample writes. The two share the word
"ring" but solve different problems; this example deliberately uses
the simpler `Vec<f32>` so the distinction is visible.

## Ports

| Index | Direction | Kind    | Name        | Default | Bounds         |
|------:|-----------|---------|-------------|---------|----------------|
| 0     | input     | audio   | In          | —       | —              |
| 1     | output    | audio   | Out         | —       | —              |
| 2     | input     | control | Delay (ms)  | 100     | [0, 2000]      |
| 3     | input     | control | Feedback    | middle  | [0, 0.95]      |
| 4     | input     | control | Mix         | middle  | [0, 1]         |

Feedback is clamped to 0.95 internally; values closer to 1.0 cause
runaway gain on a self-resonating delay line, so the framework caps
the user-set range below that threshold.

## Behaviour

For each input sample `x[n]`:

```
delayed         = buffer[(write + len − delay_samples) mod len]
output[n]       = (1 − mix) · x[n] + mix · delayed
buffer[write]   = x[n] + feedback · delayed
write           = (write + 1) mod len
```

`delay_samples = round(delay_ms · 0.001 · sample_rate)`, clamped to
`buffer.len() − 1`.

## Caveats

- Delay-time changes are sampled at the start of each `run()` call;
  large jumps will click. Real delays interpolate; this example does
  not.
- No anti-aliasing or fractional-sample interpolation. The plugin is
  meant as a framework demonstration, not a production effect.

## Build

```sh
cargo build --release -p tympan-delay
# target/release/libtympan_delay.so
```

## License

Dual-licensed under either of:

- Apache License, Version 2.0 (see [`LICENSE-APACHE`](../../LICENSE-APACHE))
- MIT license (see [`LICENSE-MIT`](../../LICENSE-MIT))

at your option.
