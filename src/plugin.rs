//! The [`Plugin`] trait — the framework's main user-facing surface.

use crate::{
    error::InstantiateError,
    port::{PortDescriptor, Ports},
    raw,
    realtime::RealtimeContext,
};

/// A LADSPA plugin written against this framework.
///
/// Implement `Plugin` on the type that holds your plugin's
/// per-instance state. The trait's associated constants and methods
/// map directly onto the LADSPA descriptor and callback table; the
/// framework wraps them in C-compatible shims and exposes a
/// `ladspa_descriptor` entry point via the
/// [`plugin_entry!`](crate::plugin_entry) macro.
///
/// # Example
///
/// ```rust,no_run
/// use tympan_ladspa::{
///     plugin_entry,
///     port::{PortDefault, PortDescriptor, Ports},
///     realtime::RealtimeContext,
///     InstantiateError, Plugin,
/// };
///
/// struct Gain;
///
/// impl Plugin for Gain {
///     const UNIQUE_ID: u32 = 0x0010_0001;
///     const LABEL: &'static str = "gain";
///     const NAME: &'static str = "Linear Gain";
///     const MAKER: &'static str = "Example";
///     const COPYRIGHT: &'static str = "MIT OR Apache-2.0";
///
///     fn ports() -> &'static [PortDescriptor] {
///         static PORTS: &[PortDescriptor] = &[
///             PortDescriptor::audio_input("In"),
///             PortDescriptor::audio_output("Out"),
///             PortDescriptor::control_input("Gain")
///                 .with_default(PortDefault::One)
///                 .with_bounds(0.0, 4.0),
///         ];
///         PORTS
///     }
///
///     fn instantiate(_sample_rate: u32) -> Result<Self, InstantiateError> {
///         Ok(Self)
///     }
///
///     fn run(&mut self, _rt: &RealtimeContext, _frames: usize, ports: &mut Ports<'_>) {
///         let gain = ports.control_input(2);
///         let (input, output) = ports.audio_in_out(0, 1);
///         for (i, o) in input.iter().zip(output.iter_mut()) {
///             *o = *i * gain;
///         }
///     }
/// }
///
/// plugin_entry!(Gain);
/// ```
///
/// # Identity invariants
///
/// Every plugin must declare a [`UNIQUE_ID`](Self::UNIQUE_ID) that is
/// globally unique among LADSPA plugins. Authors obtain one from the
/// [LADSPA central registry](https://ladspa.org/) or pick a
/// high-entropy value outside the reserved low range.
///
/// The [`LABEL`](Self::LABEL) is the short machine-readable name
/// hosts use in configuration files; both [`UNIQUE_ID`] and
/// [`LABEL`] are part of the plugin's stable ABI and must not change
/// across versions of the same plugin (see
/// [ADR 0004](https://github.com/penta2himajin/tympan-ladspa/blob/main/docs/decisions/0004-no-global-state-multi-instance.md)).
///
/// # Lifecycle
///
/// A LADSPA host calls the methods in this order:
///
/// 1. [`Plugin::instantiate`] once per instance.
/// 2. [`Plugin::activate`] zero or more times before processing
///    starts.
/// 3. [`Plugin::run`] many times. Each call processes `frames`
///    samples using the buffers bound via LADSPA's `connect_port`
///    callback (handled by the framework).
/// 4. [`Plugin::deactivate`] zero or more times, balancing each
///    `activate`.
/// 5. The instance is dropped when the host issues `cleanup`.
///
/// `run` executes on the host's realtime audio thread. Code reachable
/// from it must obey the realtime invariants documented in
/// [`CLAUDE.md`](https://github.com/penta2himajin/tympan-ladspa/blob/main/CLAUDE.md)
/// — no allocations, no `Mutex::lock`, no blocking syscalls. The
/// [`RealtimeContext`] passed to `run` is the type-level witness that
/// the caller is on the realtime thread.
pub trait Plugin: Sized + Send + 'static {
    /// Globally unique 32-bit identifier for this plugin. Obtained
    /// from the LADSPA registry or self-assigned from a sufficiently
    /// large random space.
    const UNIQUE_ID: u32;

    /// Short machine-readable name. Hosts reference plugins by label
    /// in configuration files.
    const LABEL: &'static str;

    /// Human-readable plugin name. Shown in host UIs.
    const NAME: &'static str;

    /// Plugin author or vendor.
    const MAKER: &'static str;

    /// Copyright or licence notice, displayed verbatim by hosts that
    /// surface it.
    const COPYRIGHT: &'static str;

    /// Global plugin properties — a bitwise OR of `PROPERTY_*`
    /// constants re-exported from [`crate::raw`].
    ///
    /// The framework defaults this to zero (no properties). Plugins
    /// that perform in-place processing should leave it that way;
    /// plugins that require distinct buffers for every port should
    /// set `raw::PROPERTY_INPLACE_BROKEN`. Plugins that guarantee a
    /// hard-realtime-safe `run` may additionally set
    /// `raw::PROPERTY_HARD_RT_CAPABLE`.
    const PROPERTIES: raw::Properties = 0;

    /// Static port table describing this plugin's audio and control
    /// ports.
    ///
    /// Must return a `&'static` slice so that the framework can
    /// publish the corresponding LADSPA `PortDescriptor`,
    /// `PortRangeHint`, and name arrays without copying. Typical
    /// implementations return a reference to a `static [
    /// PortDescriptor; N]` array initialised at compile time.
    fn ports() -> &'static [PortDescriptor];

    /// Construct a new plugin instance for the given sample rate.
    ///
    /// Called once per instance on the host's setup thread, *not* the
    /// realtime thread. Allocation is permitted; failure should be
    /// reported by returning [`InstantiateError`].
    fn instantiate(sample_rate: u32) -> Result<Self, InstantiateError>;

    /// Reset internal state prior to processing. Default
    /// implementation is a no-op.
    ///
    /// Called on a non-realtime thread before the first `run` and
    /// optionally between `deactivate` and the next `run`.
    fn activate(&mut self) {}

    /// Process `frames` audio frames using the buffers connected by
    /// the host.
    ///
    /// **Runs on the realtime audio thread.** Code reachable from
    /// `run` must not allocate, lock, or block. The [`RealtimeContext`]
    /// parameter is a type-level witness of this invariant and gates
    /// access to the framework's other realtime-only utilities.
    fn run(&mut self, rt: &RealtimeContext, frames: usize, ports: &mut Ports<'_>);

    /// Tear down anything `activate` set up. Default implementation
    /// is a no-op.
    fn deactivate(&mut self) {}
}
