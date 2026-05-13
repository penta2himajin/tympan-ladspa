# ADR 0005: CI verification strategy and scope boundary

- Status: Accepted
- Date: 2026-05-13

## Context

The framework imposes strong invariants — allocation-free `run()`,
no mutex usage in the realtime path, no global state, no blocking
syscalls. `CLAUDE.md` lists six explicit prohibitions and ADR 0004 adds
the no-global-state rule. The question is which of these can be
verified mechanically on GitHub-hosted Linux runners, and which must
fall back to local or self-hosted testing.

A preliminary investigation surveyed three classes of tooling:

1. The LADSPA SDK's offline utilities (`analyseplugin`, `listplugins`,
   `applyplugin`).
2. Kernel-level audio device emulation (`snd-dummy`, `snd-aloop`) and
   userspace audio servers (PipeWire with `null-audio-sink`, JACK with
   the `dummy` backend).
3. Runtime instrumentation (`assert_no_alloc`, `strace` allow-listing,
   AddressSanitizer / ThreadSanitizer).

Findings:

- **`applyplugin`** drives a plugin through the full lifecycle
  (`instantiate → connect_port → activate → run → deactivate → cleanup`)
  with a WAV file as input and writes a WAV output. It is the only
  standard tool that exercises the realtime callback path offline.
  Input/output is restricted to PCM WAV formats that `libsndfile`
  understands; the plugin's port count must match the WAV channel count.
- **`analyseplugin`** `dlopen`s the `.so` and reads the descriptor table
  without invoking `instantiate`. It validates symbol visibility,
  C ABI layout, and descriptor construction with no side effects.
- **`snd-dummy` / `snd-aloop`** cannot be loaded on GitHub-hosted
  runners: the kernel module `.ko` files are absent and
  `linux-modules-extra-$(uname -r)` packages usually mismatch the
  running kernel (tracked in
  [actions/runner-images#1114](https://github.com/actions/runner-images/issues/1114),
  unresolved).
- **PipeWire under `dbus-run-session`** can be brought up with a
  `null-audio-sink`, but the configuration is brittle and adds many
  flake vectors for a benefit (real `module-filter-chain` host)
  that `applyplugin` already approximates for LADSPA.
- **JACK's `dummy` backend** runs without root, kernel modules, or
  realtime privileges. It is the standard way to host a plugin in CI
  for LV2 (`jalv`), but LADSPA has no maintained equivalent host that
  pairs cleanly with it, so its value for this project is marginal.
- **`assert_no_alloc`** is a global-allocator wrapper that fails (or
  warns) on allocations within a guarded scope. Inserting it into an
  integration test that calls `run()` mechanically enforces prohibition
  1 from `CLAUDE.md`.
- **`strace -e trace=...`** allow-listing on an `applyplugin` invocation
  can catch syscalls in the realtime path (`futex`, `openat`, `write`,
  `mmap`), covering prohibitions 2 and 6.
- **AddressSanitizer / ThreadSanitizer** run under `cargo test` on
  nightly Rust and surface FFI-boundary UB. `miri` does not work
  across the LADSPA C ABI boundary, so it is reserved for pure-Rust
  unit tests inside the framework.

Industry baseline observed in adjacent Rust audio projects (`nih-plug`,
`rust-lv2`, the archived `ladspa` crate, Calf): CI typically stops at
`cargo build` and artifact upload. Loading the plugin and exercising
its lifecycle is left to maintainer hands. Adopting `analyseplugin` +
`applyplugin` already puts this project above that baseline.

## Decision

CI verification is organised in three tiers. Each tier defines what
runs on which trigger and what is intentionally out of scope.

### Tier 1 — `smoke` (every PR push, target < 1 minute)

- `cargo build --release` for the cdylib(s).
- `cargo test` for pure-Rust unit tests (no host involvement).
- `cargo clippy -- -D warnings`.
- `cargo fmt --check`.
- `nm -D target/release/lib*.so | grep ladspa_descriptor` to confirm
  the C ABI entry point is present and exported.

This tier blocks merge.

### Tier 2 — `standard` (every PR push and push to `main`, target < 5 min)

Tier 1 plus:

- Install `ladspa-sdk` via `apt-get`.
- `LADSPA_PATH=$PWD/target/release analyseplugin lib*.so` for every
  shipped plugin; capture stdout and diff against a golden file in
  `tests/golden/`.
- `applyplugin` runs against a committed test WAV
  (`tests/fixtures/*.wav`) for each example plugin. Output is
  inspected with `sox --i` / a small Rust verifier asserting:
  - No `NaN` or `±Inf` samples.
  - RMS within a tolerance band derived from a golden output (or a
    plugin-specific analytic bound for trivial cases like gain).
- An `assert_no_alloc`-instrumented integration test that
  `libloading::dlopen`s each cdylib, drives a synthesised buffer
  through `run()` under a no-alloc guard, and fails the build on
  violation.
- An AddressSanitizer-enabled job (`RUSTFLAGS="-Zsanitizer=address"`,
  nightly) running the same `applyplugin`-based fixtures, parallel to
  the main job.

This tier blocks merge.

### Tier 3 — `full` (nightly schedule and `workflow_dispatch`, target < 30 min)

Tier 2 plus:

- ThreadSanitizer (`-Zsanitizer=thread`) run with a multi-instance
  test that calls `instantiate` more than once concurrently from
  separate Rust threads.
- `strace` allow-listing: run `applyplugin` under
  `strace -f -e trace=futex,openat,write,mmap,brk,clock_nanosleep`
  and grep the trace against a committed allow-list. Any new syscall
  in the realtime window fails the job.
- `criterion` benchmarks measuring per-sample cost of `run()`. The
  workflow compares results against a stored baseline and fails on
  regression beyond a configured threshold.
- `cargo audit` and `cargo deny`.

This tier does not block merge but its failures open issues
automatically (or notify, depending on later infra choices).

### Explicitly out of CI scope

The following are *not* tested on GitHub-hosted runners. They are
documented in `docs/manual-verification.md` (to be created during
implementation) as a pre-release manual checklist:

- Loading the plugin under a real PipeWire session with
  `module-filter-chain`, with playback to a real or null sink.
- Loading the plugin under Ardour or Audacity.
- Realtime scheduling behaviour under `SCHED_FIFO`.
- xrun rates under sustained load.
- Behaviour with non-standard sample rates beyond the fixtures
  committed to `tests/fixtures/`.

If a self-hosted runner becomes available later, these may be
promoted into a Tier 4 without breaking changes to tiers 1–3.

## Consequences

Positive:

- Prohibitions 1, 2, and 6 from `CLAUDE.md` become mechanically
  enforceable. Prohibition 4 (no external C library beyond `ladspa.h`)
  is enforced by `cargo deny` in tier 3.
- The "no global state" rule from ADR 0004 is checked by a CI grep for
  `static mut` in `src/`, run as part of tier 1.
- Tier 2 catches the most common regression mode (a change that breaks
  descriptor construction or plugin lifecycle) on every PR without
  requiring any host beyond `applyplugin`.
- Sanitizer jobs catch FFI-boundary UB before it ships, which is the
  hardest class of bug to debug in real hosts.

Negative:

- `applyplugin` constrains test fixtures to formats `libsndfile`
  supports. Plugins with unusual port shapes (many control ports, or
  more than stereo audio) need carefully constructed WAVs.
- Tier 2's strace allow-list is brittle; a glibc update can change the
  syscall mix. It lives in tier 3, not tier 2, for that reason.
- Real-world host behaviour (PipeWire's filter-chain quirks, Ardour's
  bypass logic) is not covered. Bugs that depend on host-specific
  behaviour will not be caught until manual verification or user
  reports.

## Trigger for revisiting

Re-evaluate this strategy when any of the following holds:

- A self-hosted Linux runner with audio hardware (or with kernel
  module load privileges) becomes part of the project's CI budget.
- A bug ships that would have been caught by PipeWire-level e2e but
  was not caught by `applyplugin` + sanitizers. Document the case and
  decide whether to move PipeWire into tier 3.
- `applyplugin` proves insufficient for a new plugin's port shape, in
  which case a small project-internal LADSPA host (Rust binary using
  the framework's own `raw` layer) should be considered.

## References

- LADSPA SDK overview: https://www.ladspa.org/ladspa_sdk/overview.html
- `analyseplugin(1)`:
  https://manpages.ubuntu.com/manpages/jammy/man1/analyseplugin.1.html
- `applyplugin(1)`:
  https://manpages.ubuntu.com/manpages/focal/en/man1/applyplugin.1.html
- `actions/runner-images#1114` (snd-aloop unavailable on hosted
  runners): https://github.com/actions/runner-images/issues/1114
- `assert_no_alloc`: https://docs.rs/assert_no_alloc
- PipeWire `module-filter-chain`:
  https://docs.pipewire.org/page_module_filter_chain.html
- Adjacent CI baselines: `nih-plug`
  (https://github.com/robbert-vdh/nih-plug/blob/master/.github/workflows/build.yml),
  `rust-lv2` (https://github.com/RustAudio/rust-lv2).
