//! Minimal LADSPA gain plugin built on `tympan-ladspa`.
//!
//! Multiplies each audio frame by a control-port gain value. Useful
//! as a smoke test for the framework's FFI shim end-to-end: the
//! plugin's lifecycle exercises every callback (`instantiate`,
//! `connect_port`, `activate`, `run`, `deactivate`, `cleanup`).
//!
//! # Build
//!
//! ```sh
//! cargo build --release -p tympan-gain
//! # produces target/release/libtympan_gain.so
//! ```
//!
//! # Run under PipeWire's filter-chain
//!
//! ```sh
//! mkdir -p ~/.ladspa
//! cp target/release/libtympan_gain.so ~/.ladspa/
//! # reference `tympan_gain` (the LABEL) in
//! # ~/.config/pipewire/filter-chain.conf.d/
//! ```
//!
//! # Verify with the LADSPA SDK
//!
//! ```sh
//! sudo apt-get install -y ladspa-sdk sox
//! LADSPA_PATH=$PWD/target/release analyseplugin libtympan_gain.so
//!
//! sox -n -r 48000 -c 1 -b 16 input.wav synth 0.1 sine 440 vol 0.3
//! LADSPA_PATH=$PWD/target/release applyplugin \
//!     input.wav output.wav libtympan_gain.so tympan_gain 2.0
//! ```

use tympan_ladspa::{
    plugin_entry,
    port::{PortDefault, PortDescriptor, Ports},
    realtime::RealtimeContext,
    InstantiateError, Plugin,
};

/// Linear gain plugin. `y[n] = gain * x[n]`.
pub struct Gain;

impl Plugin for Gain {
    /// Arbitrary value outside the LADSPA central registry's typical
    /// allocations. An example plugin not intended for distribution
    /// does not coordinate with the registry; a real plugin would.
    const UNIQUE_ID: u32 = 12_345;
    const LABEL: &'static str = "tympan_gain";
    const NAME: &'static str = "Tympan Linear Gain";
    const MAKER: &'static str = "tympan-ladspa";
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

plugin_entry!(Gain);
