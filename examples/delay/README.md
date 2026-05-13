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
- Wires up the framework's `realtime::log::LogSink` so the realtime
  path can emit diagnostic events (see "Realtime logging" below)
  without allocating, locking, or blocking.

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

## Realtime logging

The plugin defines a small event enum:

```rust
pub enum DelayLogEvent {
    FeedbackClamped { requested: f32 },
}
```

and owns a `LogSink<DelayLogEvent>` constructed in `instantiate`:

```rust
let logger = LogSink::new(64, |event| {
    eprintln!("[tympan-delay] {event:?}");
});
```

`Plugin::run` calls `self.logger.log(...)` when the host writes a
`Feedback` value above the 0.95 safety cap. `LogSink::log` forwards
to a lock-free SPSC ring buffer's `try_push` — no allocation, no
syscall, no blocking on the realtime path. A background thread
spawned by `LogSink::new` drains the queue and runs the user's
closure (here, `eprintln!`).

When the LADSPA host calls `cleanup`, the framework drops the
`Delay` instance, which drops the `LogSink`, which signals
shutdown, flushes any remaining events, and joins the drainer. All
on the non-realtime cleanup thread.

You can reproduce the log output with `applyplugin`:

```sh
LADSPA_PATH=$PWD/target/release \
  applyplugin in.wav out.wav libtympan_delay.so tympan_delay 100.0 0.99 0.5
# [tympan-delay] FeedbackClamped { requested: 0.99 }
```

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
