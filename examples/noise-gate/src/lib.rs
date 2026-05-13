//! Hysteresis noise gate built on `tympan-ladspa`.
//!
//! Passes samples through unchanged while the gate is open and
//! mutes the output while the gate is closed. Two control ports
//! govern the state transitions:
//!
//! - When `|sample| > open_threshold`, the gate latches **open**.
//! - When `|sample| < close_threshold`, the gate latches **closed**.
//! - Between the two thresholds the gate stays in its previous state.
//!
//! `open_threshold` should be greater than `close_threshold`; the
//! plugin does not enforce this — a host that wires them backwards
//! will simply observe a gate that chatters at the crossover.
//!
//! This example exists to demonstrate three framework features:
//!
//! 1. Multiple control inputs, distinguished by port index.
//! 2. Per-instance state (the `is_open` latch) preserved across
//!    `run` calls.
//! 3. Plugin logic factored into a pure function (`apply_gate`) so
//!    it can be unit-tested without building a `DescriptorBundle`.
//!
//! # Build
//!
//! ```sh
//! cargo build --release -p tympan-noise-gate
//! # target/release/libtympan_noise_gate.so
//! ```

use tympan_ladspa::{
    plugin_entry,
    port::{PortDefault, PortDescriptor, Ports},
    raw::Data,
    realtime::RealtimeContext,
    InstantiateError, Plugin,
};

/// Hysteresis gate state.
pub struct NoiseGate {
    is_open: bool,
}

impl Plugin for NoiseGate {
    /// Arbitrary value, not coordinated with the LADSPA central
    /// registry — see ADR 0004 for the framework's stance on plugin
    /// identity.
    const UNIQUE_ID: u32 = 23_456;
    const LABEL: &'static str = "tympan_noise_gate";
    const NAME: &'static str = "Tympan Hysteresis Noise Gate";
    const MAKER: &'static str = "tympan-ladspa";
    const COPYRIGHT: &'static str = "MIT OR Apache-2.0";

    fn ports() -> &'static [PortDescriptor] {
        static PORTS: &[PortDescriptor] = &[
            PortDescriptor::audio_input("In"),
            PortDescriptor::audio_output("Out"),
            PortDescriptor::control_input("Open Threshold")
                .with_default(PortDefault::Middle)
                .with_bounds(0.0, 1.0),
            PortDescriptor::control_input("Close Threshold")
                .with_default(PortDefault::Low)
                .with_bounds(0.0, 1.0),
        ];
        PORTS
    }

    fn instantiate(_sample_rate: u32) -> Result<Self, InstantiateError> {
        Ok(Self { is_open: false })
    }

    fn activate(&mut self) {
        // LADSPA's `activate` callback is the spec-blessed place to
        // reset transient state before the next run cycle.
        self.is_open = false;
    }

    fn run(&mut self, _rt: &RealtimeContext, _frames: usize, ports: &mut Ports<'_>) {
        let open_threshold = ports.control_input(2);
        let close_threshold = ports.control_input(3);
        let (input, output) = ports.audio_in_out(0, 1);
        apply_gate(
            input,
            output,
            &mut self.is_open,
            open_threshold,
            close_threshold,
        );
    }
}

/// Per-sample gate application.
///
/// Factored out of [`NoiseGate::run`] so it can be unit-tested with
/// plain Rust slices, without needing to construct a LADSPA
/// [`Ports`] view. Keeps this example's tests cheap.
fn apply_gate(
    input: &[Data],
    output: &mut [Data],
    state: &mut bool,
    open_threshold: Data,
    close_threshold: Data,
) {
    for (in_sample, out_sample) in input.iter().zip(output.iter_mut()) {
        let abs = in_sample.abs();
        if abs > open_threshold {
            *state = true;
        } else if abs < close_threshold {
            *state = false;
        }
        *out_sample = if *state { *in_sample } else { 0.0 };
    }
}

plugin_entry!(NoiseGate);

#[cfg(test)]
mod tests {
    use super::*;

    fn run_one(input: &[Data], open_th: Data, close_th: Data, initial_open: bool) -> Vec<Data> {
        let mut state = initial_open;
        let mut output = vec![0.0; input.len()];
        apply_gate(input, &mut output, &mut state, open_th, close_th);
        output
    }

    #[test]
    fn loud_signal_opens_the_gate_and_passes_through() {
        let out = run_one(&[0.8, 0.7, 0.9], 0.5, 0.1, false);
        assert_eq!(out, vec![0.8, 0.7, 0.9]);
    }

    #[test]
    fn quiet_signal_keeps_the_gate_shut() {
        let out = run_one(&[0.05, -0.05, 0.04], 0.5, 0.1, false);
        assert_eq!(out, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn hysteresis_band_preserves_open_state_once_latched() {
        // Single loud sample opens the gate, then a run of
        // mid-amplitude samples (above close_th, below open_th)
        // should continue passing.
        let out = run_one(&[0.8, 0.3, 0.3, 0.3], 0.5, 0.1, false);
        assert_eq!(out, vec![0.8, 0.3, 0.3, 0.3]);
    }

    #[test]
    fn closed_state_persists_through_mid_band() {
        // A sample below `close_threshold` keeps the gate shut.
        // Subsequent samples in the hysteresis band (between
        // `close_threshold` and `open_threshold`) must not re-open
        // it.
        let out = run_one(&[0.05, 0.3, 0.3, 0.3], 0.5, 0.1, false);
        assert_eq!(out, vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn transition_open_then_close() {
        // Loud sample opens; very quiet sample closes; subsequent
        // mid-band stays closed.
        let out = run_one(&[0.8, 0.0, 0.3], 0.5, 0.1, false);
        assert_eq!(out, vec![0.8, 0.0, 0.0]);
    }

    #[test]
    fn activate_resets_state() {
        let mut gate = NoiseGate::instantiate(48_000).unwrap();
        gate.is_open = true;
        gate.activate();
        assert!(!gate.is_open);
    }
}
