//! Primitive types and bitfield constants from `ladspa.h`.

use core::ffi::{c_int, c_void};

/// Sample value type. Corresponds to `LADSPA_Data` (`float` in
/// `ladspa.h`).
///
/// LADSPA fixes the audio sample format at 32-bit floating point. Both
/// audio and control port buffers carry values of this type.
pub type Data = f32;

/// Opaque handle returned by a plugin's `instantiate()` callback and
/// passed back to every subsequent lifecycle call. Corresponds to
/// `LADSPA_Handle` (`void *` in `ladspa.h`).
pub type Handle = *mut c_void;

/// Bitfield describing global plugin properties. Corresponds to
/// `LADSPA_Properties` (`int` in `ladspa.h`).
///
/// Compose values with the `PROPERTY_*` constants in this module.
pub type Properties = c_int;

/// `LADSPA_PROPERTY_REALTIME` — the plugin author declares the plugin
/// is intended for realtime operation. Informational only; does not
/// alter host behaviour.
pub const PROPERTY_REALTIME: Properties = 0x1;

/// `LADSPA_PROPERTY_INPLACE_BROKEN` — the plugin cannot tolerate input
/// and output port buffers pointing at the same memory. Hosts must
/// allocate distinct buffers for each port.
pub const PROPERTY_INPLACE_BROKEN: Properties = 0x2;

/// `LADSPA_PROPERTY_HARD_RT_CAPABLE` — the plugin guarantees its
/// realtime path performs no blocking operations and is safe to run on
/// a hard-realtime audio thread.
pub const PROPERTY_HARD_RT_CAPABLE: Properties = 0x4;

/// Bitfield describing the role of a single port. Corresponds to
/// `LADSPA_PortDescriptor` (`int` in `ladspa.h`).
///
/// A valid port descriptor combines exactly one direction bit
/// (`PORT_INPUT` or `PORT_OUTPUT`) with exactly one kind bit
/// (`PORT_CONTROL` or `PORT_AUDIO`).
pub type PortDescriptor = c_int;

/// `LADSPA_PORT_INPUT` — port carries data from the host into the
/// plugin.
pub const PORT_INPUT: PortDescriptor = 0x1;

/// `LADSPA_PORT_OUTPUT` — port carries data from the plugin to the
/// host.
pub const PORT_OUTPUT: PortDescriptor = 0x2;

/// `LADSPA_PORT_CONTROL` — port holds a single scalar control value.
/// The host writes (or the plugin writes, for control outputs) a
/// single [`Data`] per port, not a buffer.
pub const PORT_CONTROL: PortDescriptor = 0x4;

/// `LADSPA_PORT_AUDIO` — port references a buffer of `SampleCount`
/// [`Data`] values supplied by the host on each `run()` call.
pub const PORT_AUDIO: PortDescriptor = 0x8;

/// Bitfield describing the value range and default of a port.
/// Corresponds to `LADSPA_PortRangeHintDescriptor` (`int` in
/// `ladspa.h`).
///
/// The low bits encode bounds, scaling, and integer/toggled
/// constraints; the `HINT_DEFAULT_MASK` window selects one of the
/// `HINT_DEFAULT_*` symbolic defaults.
pub type PortRangeHintDescriptor = c_int;

/// `LADSPA_HINT_BOUNDED_BELOW` — `LowerBound` of the
/// [`PortRangeHint`](super::PortRangeHint) is meaningful.
pub const HINT_BOUNDED_BELOW: PortRangeHintDescriptor = 0x1;

/// `LADSPA_HINT_BOUNDED_ABOVE` — `UpperBound` of the
/// [`PortRangeHint`](super::PortRangeHint) is meaningful.
pub const HINT_BOUNDED_ABOVE: PortRangeHintDescriptor = 0x2;

/// `LADSPA_HINT_TOGGLED` — port acts as a boolean. Values `<= 0`
/// represent off, `> 0` on.
pub const HINT_TOGGLED: PortRangeHintDescriptor = 0x4;

/// `LADSPA_HINT_SAMPLE_RATE` — both bounds are multiplied by the
/// instantiated sample rate before being interpreted.
pub const HINT_SAMPLE_RATE: PortRangeHintDescriptor = 0x8;

/// `LADSPA_HINT_LOGARITHMIC` — the host's UI for this control should
/// use a logarithmic scale.
pub const HINT_LOGARITHMIC: PortRangeHintDescriptor = 0x10;

/// `LADSPA_HINT_INTEGER` — only integer values are meaningful for this
/// port.
pub const HINT_INTEGER: PortRangeHintDescriptor = 0x20;

/// `LADSPA_HINT_DEFAULT_MASK` — bitmask covering the symbolic default
/// values below.
pub const HINT_DEFAULT_MASK: PortRangeHintDescriptor = 0x3C0;

/// `LADSPA_HINT_DEFAULT_NONE` — no default is specified.
pub const HINT_DEFAULT_NONE: PortRangeHintDescriptor = 0x0;

/// `LADSPA_HINT_DEFAULT_MINIMUM` — default is `LowerBound`.
pub const HINT_DEFAULT_MINIMUM: PortRangeHintDescriptor = 0x40;

/// `LADSPA_HINT_DEFAULT_LOW` — default is roughly the lower quartile of
/// the range. The exact formula is host-defined.
pub const HINT_DEFAULT_LOW: PortRangeHintDescriptor = 0x80;

/// `LADSPA_HINT_DEFAULT_MIDDLE` — default is roughly the centre of the
/// range. The exact formula is host-defined.
pub const HINT_DEFAULT_MIDDLE: PortRangeHintDescriptor = 0xC0;

/// `LADSPA_HINT_DEFAULT_HIGH` — default is roughly the upper quartile
/// of the range. The exact formula is host-defined.
pub const HINT_DEFAULT_HIGH: PortRangeHintDescriptor = 0x100;

/// `LADSPA_HINT_DEFAULT_MAXIMUM` — default is `UpperBound`.
pub const HINT_DEFAULT_MAXIMUM: PortRangeHintDescriptor = 0x140;

/// `LADSPA_HINT_DEFAULT_0` — default is the literal value `0`.
pub const HINT_DEFAULT_0: PortRangeHintDescriptor = 0x200;

/// `LADSPA_HINT_DEFAULT_1` — default is the literal value `1`.
pub const HINT_DEFAULT_1: PortRangeHintDescriptor = 0x240;

/// `LADSPA_HINT_DEFAULT_100` — default is the literal value `100`.
pub const HINT_DEFAULT_100: PortRangeHintDescriptor = 0x280;

/// `LADSPA_HINT_DEFAULT_440` — default is the literal value `440`
/// (the conventional concert-pitch tuning frequency).
pub const HINT_DEFAULT_440: PortRangeHintDescriptor = 0x2C0;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn data_is_f32() {
        assert_eq!(size_of::<Data>(), 4);
    }

    #[test]
    fn handle_is_pointer_sized() {
        assert_eq!(size_of::<Handle>(), size_of::<usize>());
    }

    #[test]
    fn port_direction_bits_distinct() {
        assert_ne!(PORT_INPUT, PORT_OUTPUT);
        assert_eq!(PORT_INPUT & PORT_OUTPUT, 0);
    }

    #[test]
    fn port_kind_bits_distinct() {
        assert_ne!(PORT_CONTROL, PORT_AUDIO);
        assert_eq!(PORT_CONTROL & PORT_AUDIO, 0);
    }

    #[test]
    fn hint_default_constants_fit_mask() {
        for d in [
            HINT_DEFAULT_NONE,
            HINT_DEFAULT_MINIMUM,
            HINT_DEFAULT_LOW,
            HINT_DEFAULT_MIDDLE,
            HINT_DEFAULT_HIGH,
            HINT_DEFAULT_MAXIMUM,
            HINT_DEFAULT_0,
            HINT_DEFAULT_1,
            HINT_DEFAULT_100,
            HINT_DEFAULT_440,
        ] {
            assert_eq!(d & !HINT_DEFAULT_MASK, 0, "default {d:#x} escapes the mask");
        }
    }

    #[test]
    fn hint_bounds_outside_default_mask() {
        for b in [
            HINT_BOUNDED_BELOW,
            HINT_BOUNDED_ABOVE,
            HINT_TOGGLED,
            HINT_SAMPLE_RATE,
            HINT_LOGARITHMIC,
            HINT_INTEGER,
        ] {
            assert_eq!(
                b & HINT_DEFAULT_MASK,
                0,
                "bound bit {b:#x} overlaps default mask"
            );
        }
    }
}
