//! Port descriptors and runtime port access.
//!
//! Each LADSPA plugin declares a fixed set of ports at compile time:
//! audio buffers and control scalars, each either an input or an
//! output. [`PortDescriptor`] is the compile-time declaration;
//! [`Ports`] is the runtime view the framework hands to
//! [`Plugin::run`](crate::Plugin::run) so the user can read inputs and
//! write outputs.

use core::slice;

use crate::raw::{
    self, Data, HINT_BOUNDED_ABOVE, HINT_BOUNDED_BELOW, HINT_DEFAULT_0, HINT_DEFAULT_1,
    HINT_DEFAULT_100, HINT_DEFAULT_440, HINT_INTEGER, HINT_LOGARITHMIC, HINT_SAMPLE_RATE,
    HINT_TOGGLED,
};

/// The four possible port roles in LADSPA: data flowing in or out of
/// the plugin, carrying either an audio buffer or a scalar control
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortKind {
    /// Buffer of [`Data`] samples flowing from host to plugin.
    AudioInput,
    /// Buffer of [`Data`] samples flowing from plugin to host.
    AudioOutput,
    /// Single [`Data`] scalar provided by the host (a knob, slider,
    /// or automation value).
    ControlInput,
    /// Single [`Data`] scalar written by the plugin (a level meter,
    /// detected pitch, etc.).
    ControlOutput,
}

impl PortKind {
    /// Returns the bitwise OR of LADSPA's direction and kind
    /// descriptors corresponding to this `PortKind`.
    pub(crate) const fn ladspa_bits(self) -> raw::PortDescriptor {
        match self {
            Self::AudioInput => raw::PORT_AUDIO | raw::PORT_INPUT,
            Self::AudioOutput => raw::PORT_AUDIO | raw::PORT_OUTPUT,
            Self::ControlInput => raw::PORT_CONTROL | raw::PORT_INPUT,
            Self::ControlOutput => raw::PORT_CONTROL | raw::PORT_OUTPUT,
        }
    }
}

/// Range, default-value, and scaling hints attached to a port.
///
/// Hints are LADSPA's mechanism for communicating port semantics to
/// the host's UI (slider ranges, log scales, toggle controls, etc.).
/// Audio ports rarely have meaningful hints; control ports usually
/// do.
///
/// Construct a `PortHints` through the chained `PortDescriptor`
/// methods ([`PortDescriptor::default`], [`PortDescriptor::bounds`],
/// and friends) rather than building one directly. The fields are
/// kept crate-private so that the LADSPA bit layout never leaks into
/// user code.
#[derive(Debug, Clone, Copy)]
pub struct PortHints {
    pub(crate) descriptor: raw::PortRangeHintDescriptor,
    pub(crate) lower: Data,
    pub(crate) upper: Data,
}

impl PortHints {
    /// A `PortHints` carrying no information. Equivalent to the LADSPA
    /// "no hint" descriptor with both bounds at zero.
    pub const EMPTY: Self = Self {
        descriptor: 0,
        lower: 0.0,
        upper: 0.0,
    };
}

impl Default for PortHints {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Compile-time declaration of a single LADSPA port.
///
/// `PortDescriptor` values populate the `&'static [PortDescriptor]`
/// slice returned by [`Plugin::ports`](crate::Plugin::ports). The
/// constructors and builder methods are `const fn`, so an entire port
/// table can live in a `static` item:
///
/// ```rust
/// use tympan_ladspa::port::{PortDefault, PortDescriptor};
///
/// static PORTS: &[PortDescriptor] = &[
///     PortDescriptor::audio_input("In"),
///     PortDescriptor::audio_output("Out"),
///     PortDescriptor::control_input("Gain")
///         .with_default(PortDefault::One)
///         .with_bounds(0.0, 4.0),
/// ];
/// # let _ = PORTS;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PortDescriptor {
    pub(crate) name: &'static str,
    pub(crate) kind: PortKind,
    pub(crate) hints: PortHints,
}

impl PortDescriptor {
    /// Declare an audio input port. Audio ports carry a buffer of
    /// `frames` samples on each [`Plugin::run`](crate::Plugin::run)
    /// call.
    pub const fn audio_input(name: &'static str) -> Self {
        Self {
            name,
            kind: PortKind::AudioInput,
            hints: PortHints::EMPTY,
        }
    }

    /// Declare an audio output port.
    pub const fn audio_output(name: &'static str) -> Self {
        Self {
            name,
            kind: PortKind::AudioOutput,
            hints: PortHints::EMPTY,
        }
    }

    /// Declare a control input port. The host supplies a single
    /// [`Data`] scalar (one per [`Plugin::run`](crate::Plugin::run)
    /// call) — typically a knob position, a slider value, or an
    /// automation lane sample.
    pub const fn control_input(name: &'static str) -> Self {
        Self {
            name,
            kind: PortKind::ControlInput,
            hints: PortHints::EMPTY,
        }
    }

    /// Declare a control output port. The plugin writes a single
    /// [`Data`] scalar — typically a level meter reading, a detected
    /// frequency, or a similar measurement.
    pub const fn control_output(name: &'static str) -> Self {
        Self {
            name,
            kind: PortKind::ControlOutput,
            hints: PortHints::EMPTY,
        }
    }

    /// Set the port's lower and upper bounds.
    ///
    /// LADSPA hosts use the bounds to scale sliders and to clamp
    /// automation values. The bounds are advisory: nothing prevents a
    /// host from supplying a value outside them, and the plugin
    /// must remain stable when that happens.
    pub const fn with_bounds(mut self, lower: f32, upper: f32) -> Self {
        self.hints.descriptor |= HINT_BOUNDED_BELOW | HINT_BOUNDED_ABOVE;
        self.hints.lower = lower;
        self.hints.upper = upper;
        self
    }

    /// Declare a default value for the port.
    ///
    /// LADSPA's hint scheme can only express nine specific defaults
    /// (the variants of [`PortDefault`]). The framework therefore
    /// takes an enum instead of an arbitrary `f32`: the API is then
    /// exact rather than silently dropping unrepresentable values.
    pub const fn with_default(mut self, default: PortDefault) -> Self {
        self.hints.descriptor |= default.bits();
        self
    }

    /// Mark the port as carrying integer-valued samples. Hosts may
    /// quantise UI controls accordingly.
    pub const fn integer(mut self) -> Self {
        self.hints.descriptor |= HINT_INTEGER;
        self
    }

    /// Mark the port as a toggle. Values `<= 0` represent off, `> 0`
    /// on. Hosts typically render this as a checkbox.
    pub const fn toggled(mut self) -> Self {
        self.hints.descriptor |= HINT_TOGGLED;
        self
    }

    /// Mark the port's value as best presented on a logarithmic scale
    /// in the host's UI. Common for frequencies and amplitude knobs.
    pub const fn logarithmic(mut self) -> Self {
        self.hints.descriptor |= HINT_LOGARITHMIC;
        self
    }

    /// Mark the port's bounds as being expressed relative to the
    /// instantiated sample rate. The host multiplies the bounds by
    /// the sample rate before interpreting them.
    pub const fn sample_rate(mut self) -> Self {
        self.hints.descriptor |= HINT_SAMPLE_RATE;
        self
    }

    /// The port's human-readable name as declared by the plugin
    /// author.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The port's role.
    pub const fn kind(&self) -> PortKind {
        self.kind
    }

    /// The port's hint bundle.
    pub const fn hints(&self) -> &PortHints {
        &self.hints
    }
}

/// Default-value variants supported by LADSPA's hint scheme.
///
/// LADSPA expresses a port's default in one of nine ways: four
/// literal numeric values, four positions relative to the port's
/// declared bounds, and one "lower quartile" position. Pass a
/// `PortDefault` to [`PortDescriptor::with_default`] to set the
/// default. Plugins that need a default not in this list must
/// document it elsewhere — LADSPA's hint scheme cannot encode it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortDefault {
    /// `LADSPA_HINT_DEFAULT_MINIMUM` — default is the lower bound.
    Minimum,
    /// `LADSPA_HINT_DEFAULT_LOW` — approximately the lower quartile of
    /// the range. The exact formula is host-defined.
    Low,
    /// `LADSPA_HINT_DEFAULT_MIDDLE` — approximately the centre of the
    /// range.
    Middle,
    /// `LADSPA_HINT_DEFAULT_HIGH` — approximately the upper quartile
    /// of the range.
    High,
    /// `LADSPA_HINT_DEFAULT_MAXIMUM` — default is the upper bound.
    Maximum,
    /// `LADSPA_HINT_DEFAULT_0` — literal `0`.
    Zero,
    /// `LADSPA_HINT_DEFAULT_1` — literal `1`.
    One,
    /// `LADSPA_HINT_DEFAULT_100` — literal `100`.
    Hundred,
    /// `LADSPA_HINT_DEFAULT_440` — literal `440` (concert pitch in Hz).
    Hz440,
}

impl PortDefault {
    pub(crate) const fn bits(self) -> raw::PortRangeHintDescriptor {
        match self {
            Self::Minimum => raw::HINT_DEFAULT_MINIMUM,
            Self::Low => raw::HINT_DEFAULT_LOW,
            Self::Middle => raw::HINT_DEFAULT_MIDDLE,
            Self::High => raw::HINT_DEFAULT_HIGH,
            Self::Maximum => raw::HINT_DEFAULT_MAXIMUM,
            Self::Zero => HINT_DEFAULT_0,
            Self::One => HINT_DEFAULT_1,
            Self::Hundred => HINT_DEFAULT_100,
            Self::Hz440 => HINT_DEFAULT_440,
        }
    }
}

/// Runtime view of a plugin instance's connected port buffers.
///
/// The framework constructs `Ports` immediately before invoking
/// [`Plugin::run`](crate::Plugin::run) and discards it immediately
/// after. Each accessor maps a port index — the position of the port
/// in [`Plugin::ports`](crate::Plugin::ports) — to a slice or scalar
/// view of the buffer the host bound via LADSPA's `connect_port`
/// callback.
///
/// # Borrowing pattern
///
/// `audio_input` and `control_input` take `&self`; multiple read
/// borrows can coexist. `audio_output` and `control_output` take
/// `&mut self`, statically preventing two simultaneous mutable
/// views into the same `Ports` value.
///
/// The common pattern of "read one input, write one output" is
/// supplied by [`audio_in_out`](Self::audio_in_out), which returns
/// disjoint slices in a single call. This lets the borrow checker
/// see that the input and output slices reference different
/// host-owned buffers (a guarantee LADSPA gives when
/// `LADSPA_PROPERTY_INPLACE_BROKEN` is set — see
/// [`Plugin::PROPERTIES`](crate::Plugin::PROPERTIES)).
pub struct Ports<'host> {
    pub(crate) ptrs: &'host [*mut Data],
    pub(crate) descriptors: &'static [PortDescriptor],
    pub(crate) frames: usize,
}

impl<'host> Ports<'host> {
    /// Number of audio frames in this `run()` invocation.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Borrow the audio buffer connected to the given input port.
    ///
    /// # Panics
    ///
    /// Panics if `port` is out of bounds or if the port's declared
    /// [`PortKind`] is not [`PortKind::AudioInput`].
    pub fn audio_input(&self, port: usize) -> &[Data] {
        self.assert_kind(port, PortKind::AudioInput);
        let ptr = self.ptrs[port];
        // SAFETY: the host guarantees that the buffer at `ptr` holds
        // at least `self.frames` `Data` samples for the lifetime of
        // the `run` callback. The lifetime `'host` outlives this
        // borrow; `&self` precludes mutation through `ports` during
        // the borrow.
        unsafe { slice::from_raw_parts(ptr as *const Data, self.frames) }
    }

    /// Mutably borrow the audio buffer connected to the given output
    /// port.
    ///
    /// # Panics
    ///
    /// Panics if `port` is out of bounds or if the port's declared
    /// [`PortKind`] is not [`PortKind::AudioOutput`].
    pub fn audio_output(&mut self, port: usize) -> &mut [Data] {
        self.assert_kind(port, PortKind::AudioOutput);
        let ptr = self.ptrs[port];
        // SAFETY: see `audio_input`. The `&mut self` receiver
        // statically prevents a concurrent mutable borrow of the same
        // port via this `Ports` value.
        unsafe { slice::from_raw_parts_mut(ptr, self.frames) }
    }

    /// Read the scalar value supplied to the given control input
    /// port.
    ///
    /// # Panics
    ///
    /// Panics if `port` is out of bounds or if the port's declared
    /// [`PortKind`] is not [`PortKind::ControlInput`].
    pub fn control_input(&self, port: usize) -> Data {
        self.assert_kind(port, PortKind::ControlInput);
        let ptr = self.ptrs[port];
        // SAFETY: the host bound `ptr` to a single `Data` scalar
        // before invoking `run`. The pointer is non-null and aligned
        // by LADSPA's contract.
        unsafe { *(ptr as *const Data) }
    }

    /// Mutably borrow the scalar slot connected to the given control
    /// output port. Writing through the returned reference signals
    /// the value to the host.
    ///
    /// # Panics
    ///
    /// Panics if `port` is out of bounds or if the port's declared
    /// [`PortKind`] is not [`PortKind::ControlOutput`].
    pub fn control_output(&mut self, port: usize) -> &mut Data {
        self.assert_kind(port, PortKind::ControlOutput);
        let ptr = self.ptrs[port];
        // SAFETY: `ptr` is a non-null, aligned `Data` slot bound by
        // the host. `&mut self` rules out concurrent borrows.
        unsafe { &mut *ptr }
    }

    /// Borrow an audio input slice and an audio output slice
    /// simultaneously. The typical in-place processing pattern.
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds, if `in_port` and
    /// `out_port` are equal, or if the ports do not have the expected
    /// kinds.
    pub fn audio_in_out(&mut self, in_port: usize, out_port: usize) -> (&[Data], &mut [Data]) {
        assert_ne!(
            in_port, out_port,
            "audio_in_out: input and output port indices must differ"
        );
        self.assert_kind(in_port, PortKind::AudioInput);
        self.assert_kind(out_port, PortKind::AudioOutput);
        let in_ptr = self.ptrs[in_port];
        let out_ptr = self.ptrs[out_port];
        // SAFETY: LADSPA hosts that respect
        // `LADSPA_PROPERTY_INPLACE_BROKEN` (the default for
        // `Plugin::PROPERTIES` in this framework) guarantee distinct
        // buffers for distinct ports. The runtime `assert_ne!` above
        // additionally rules out the trivial case where the caller
        // names the same port for both.
        unsafe {
            let input = slice::from_raw_parts(in_ptr as *const Data, self.frames);
            let output = slice::from_raw_parts_mut(out_ptr, self.frames);
            (input, output)
        }
    }

    fn assert_kind(&self, port: usize, expected: PortKind) {
        let actual = self.descriptors[port].kind;
        assert_eq!(
            actual, expected,
            "port {port} is declared as {actual:?}, but accessed as {expected:?}",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_constructors_compose_into_static_table() {
        // The point of `const fn` constructors is that an entire port
        // table can live in static storage.
        static PORTS: &[PortDescriptor] = &[
            PortDescriptor::audio_input("In"),
            PortDescriptor::audio_output("Out"),
            PortDescriptor::control_input("Gain")
                .with_default(PortDefault::One)
                .with_bounds(0.0, 4.0),
        ];

        assert_eq!(PORTS.len(), 3);
        assert_eq!(PORTS[0].kind(), PortKind::AudioInput);
        assert_eq!(PORTS[0].name(), "In");
        assert_eq!(PORTS[1].kind(), PortKind::AudioOutput);
        assert_eq!(PORTS[2].kind(), PortKind::ControlInput);
        let gain = &PORTS[2];
        assert_eq!(gain.hints().descriptor & HINT_DEFAULT_1, HINT_DEFAULT_1,);
        assert_eq!(gain.hints().lower, 0.0);
        assert_eq!(gain.hints().upper, 4.0);
    }

    #[test]
    fn ladspa_bits_match_spec() {
        assert_eq!(
            PortKind::AudioInput.ladspa_bits(),
            raw::PORT_AUDIO | raw::PORT_INPUT
        );
        assert_eq!(
            PortKind::ControlOutput.ladspa_bits(),
            raw::PORT_CONTROL | raw::PORT_OUTPUT
        );
    }

    #[test]
    fn literal_default_encodes_correctly() {
        let p = PortDescriptor::control_input("Freq").with_default(PortDefault::Hz440);
        assert_eq!(
            p.hints().descriptor & raw::HINT_DEFAULT_MASK,
            raw::HINT_DEFAULT_440,
        );
    }

    #[test]
    fn symbolic_default_encodes_correctly() {
        let p = PortDescriptor::control_input("Q")
            .with_bounds(0.0, 1.0)
            .with_default(PortDefault::Middle);
        assert_eq!(
            p.hints().descriptor & raw::HINT_DEFAULT_MASK,
            raw::HINT_DEFAULT_MIDDLE,
        );
    }

    #[test]
    fn modifier_bits_compose() {
        let p = PortDescriptor::control_input("Freq")
            .with_bounds(20.0, 20_000.0)
            .logarithmic()
            .sample_rate();
        let d = p.hints().descriptor;
        assert!(d & HINT_BOUNDED_BELOW != 0);
        assert!(d & HINT_BOUNDED_ABOVE != 0);
        assert!(d & HINT_LOGARITHMIC != 0);
        assert!(d & HINT_SAMPLE_RATE != 0);
        assert!(d & HINT_TOGGLED == 0);
    }

    #[test]
    fn ports_runtime_access_pattern() {
        // Drive the runtime accessors with stack-allocated buffers in
        // place of host-owned ones to exercise the slicing paths.
        static DESCS: &[PortDescriptor] = &[
            PortDescriptor::audio_input("In"),
            PortDescriptor::audio_output("Out"),
            PortDescriptor::control_input("Gain"),
        ];

        let mut input_buf: [Data; 4] = [0.5, 1.0, 1.5, 2.0];
        let mut output_buf: [Data; 4] = [0.0; 4];
        let mut gain: Data = 2.0;

        let ptrs: [*mut Data; 3] = [
            input_buf.as_mut_ptr(),
            output_buf.as_mut_ptr(),
            &mut gain as *mut Data,
        ];

        let mut ports = Ports {
            ptrs: &ptrs,
            descriptors: DESCS,
            frames: input_buf.len(),
        };

        let g = ports.control_input(2);
        assert_eq!(g, 2.0);

        let (i, o) = ports.audio_in_out(0, 1);
        for (idx, (&sample, slot)) in i.iter().zip(o.iter_mut()).enumerate() {
            *slot = sample * g;
            let _ = idx;
        }

        assert_eq!(output_buf, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    #[should_panic(expected = "port 0 is declared as AudioInput")]
    fn wrong_kind_access_panics() {
        static DESCS: &[PortDescriptor] = &[PortDescriptor::audio_input("In")];
        let mut buf: [Data; 1] = [0.0];
        let ptrs: [*mut Data; 1] = [buf.as_mut_ptr()];
        let ports = Ports {
            ptrs: &ptrs,
            descriptors: DESCS,
            frames: 1,
        };
        let _ = ports.control_input(0);
    }
}
