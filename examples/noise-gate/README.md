# tympan-noise-gate

A hysteresis noise gate built on
[`tympan-ladspa`](../../). Passes audio through unchanged when the
gate is open and mutes the output when it is closed. State
transitions use two independent thresholds so the gate does not
chatter on signals hovering near a single boundary.

This crate exists primarily as a worked example that exercises the
framework on a non-trivial DSP shape:

- Multiple control inputs (two distinct thresholds).
- Per-instance state preserved across `run` calls (the `is_open`
  latch).
- Plugin logic factored into a pure function so it can be unit-tested
  with plain Rust slices, without building a LADSPA
  `DescriptorBundle`.

## Ports

| Index | Direction | Kind    | Name             | Default | Bounds     |
|------:|-----------|---------|------------------|---------|------------|
| 0     | input     | audio   | In               | —       | —          |
| 1     | output    | audio   | Out              | —       | —          |
| 2     | input     | control | Open Threshold   | middle  | [0.0, 1.0] |
| 3     | input     | control | Close Threshold  | low     | [0.0, 1.0] |

The host is expected to set `Close Threshold < Open Threshold`. If
they are equal or reversed, the gate degenerates to a single-
threshold gate that may chatter on noisy crossings.

## Behaviour

For each sample `x[n]`:

```
if |x[n]| > open_threshold:   state ← open
elif |x[n]| < close_threshold: state ← closed
y[n] = x[n] if state == open else 0
```

`activate()` resets the state to closed, per the LADSPA convention
that `activate` is a "reset transient state" callback.

## Build

```sh
cargo build --release -p tympan-noise-gate
# target/release/libtympan_noise_gate.so
```

## License

Dual-licensed under either of:

- Apache License, Version 2.0 (see [`LICENSE-APACHE`](../../LICENSE-APACHE))
- MIT license (see [`LICENSE-MIT`](../../LICENSE-MIT))

at your option.
