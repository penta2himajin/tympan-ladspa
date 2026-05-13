//! The `LADSPA_Descriptor` struct and surrounding aggregates.

use core::ffi::{c_char, c_ulong, c_void};

use super::types::{Data, Handle, PortDescriptor, PortRangeHintDescriptor, Properties};

/// Lower and upper bounds plus hint flags for a single port.
/// Corresponds to `LADSPA_PortRangeHint` in `ladspa.h`.
///
/// Fields are interpreted according to `hint_descriptor`:
/// - `LowerBound` is only meaningful if [`HINT_BOUNDED_BELOW`] is set.
/// - `UpperBound` is only meaningful if [`HINT_BOUNDED_ABOVE`] is set.
/// - When [`HINT_SAMPLE_RATE`] is set, both bounds are multiplied by
///   the host's sample rate before use.
///
/// [`HINT_BOUNDED_BELOW`]: super::HINT_BOUNDED_BELOW
/// [`HINT_BOUNDED_ABOVE`]: super::HINT_BOUNDED_ABOVE
/// [`HINT_SAMPLE_RATE`]: super::HINT_SAMPLE_RATE
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PortRangeHint {
    /// `HintDescriptor` field — bitfield of `HINT_*` flags.
    pub hint_descriptor: PortRangeHintDescriptor,
    /// `LowerBound` field.
    pub lower_bound: Data,
    /// `UpperBound` field.
    pub upper_bound: Data,
}

/// Plugin descriptor returned by `ladspa_descriptor()`. Corresponds to
/// `LADSPA_Descriptor` in `ladspa.h`.
///
/// LADSPA hosts query a plugin shared object by calling
/// `ladspa_descriptor(index)` and reading the struct fields it returns.
/// All pointer fields must remain valid for the lifetime of the
/// host's reference to the descriptor (typically the lifetime of the
/// `.so` load).
///
/// The framework constructs and emits instances of this struct on
/// behalf of `Plugin` implementors; user code does not interact with
/// it directly.
#[repr(C)]
pub struct Descriptor {
    /// `UniqueID` field. Globally unique identifier assigned to this
    /// plugin (see `ladspa.org` registry conventions).
    pub unique_id: c_ulong,

    /// `Label` field. Short machine-readable identifier; nul-terminated.
    pub label: *const c_char,

    /// `Properties` field. Bitfield of `PROPERTY_*` flags.
    pub properties: Properties,

    /// `Name` field. Human-readable plugin name; nul-terminated.
    pub name: *const c_char,

    /// `Maker` field. Plugin author or vendor name; nul-terminated.
    pub maker: *const c_char,

    /// `Copyright` field. Copyright or licence notice; nul-terminated.
    pub copyright: *const c_char,

    /// `PortCount` field. Number of entries in `port_descriptors`,
    /// `port_names`, and `port_range_hints`.
    pub port_count: c_ulong,

    /// `PortDescriptors` field. Pointer to `port_count` entries.
    pub port_descriptors: *const PortDescriptor,

    /// `PortNames` field. Pointer to `port_count` nul-terminated
    /// C-string pointers.
    pub port_names: *const *const c_char,

    /// `PortRangeHints` field. Pointer to `port_count`
    /// [`PortRangeHint`] entries.
    pub port_range_hints: *const PortRangeHint,

    /// `ImplementationData` field. Opaque host- or plugin-defined
    /// pointer. LADSPA does not specify its meaning.
    pub implementation_data: *mut c_void,

    /// `instantiate` callback. Constructs a new plugin instance for
    /// the given sample rate and returns its opaque handle, or NULL
    /// on failure.
    pub instantiate:
        Option<unsafe extern "C" fn(descriptor: *const Descriptor, sample_rate: c_ulong) -> Handle>,

    /// `connect_port` callback. Binds port index `port` of `instance`
    /// to the buffer or scalar location at `data_location`. Called by
    /// the host before `activate`.
    pub connect_port:
        Option<unsafe extern "C" fn(instance: Handle, port: c_ulong, data_location: *mut Data)>,

    /// `activate` callback. Optional. Resets internal state in
    /// preparation for processing. May be NULL.
    pub activate: Option<unsafe extern "C" fn(instance: Handle)>,

    /// `run` callback. Mandatory. Processes `sample_count` frames
    /// using the buffers previously bound via `connect_port`.
    pub run: Option<unsafe extern "C" fn(instance: Handle, sample_count: c_ulong)>,

    /// `run_adding` callback. Optional accumulating variant of `run`;
    /// hosts fall back to `run` when this is NULL.
    ///
    /// `tympan-ladspa` does not expose this; see
    /// [ADR 0001](https://github.com/penta2himajin/tympan-ladspa/blob/main/docs/decisions/0001-skip-run-adding.md).
    /// Descriptors produced by this framework leave the field as
    /// `None`.
    pub run_adding: Option<unsafe extern "C" fn(instance: Handle, sample_count: c_ulong)>,

    /// `set_run_adding_gain` callback. Companion to `run_adding`. Left
    /// `None` by this framework — see [`run_adding`](Self::run_adding).
    pub set_run_adding_gain: Option<unsafe extern "C" fn(instance: Handle, gain: Data)>,

    /// `deactivate` callback. Optional counterpart to `activate`. May
    /// be NULL.
    pub deactivate: Option<unsafe extern "C" fn(instance: Handle)>,

    /// `cleanup` callback. Mandatory. Destroys the instance and
    /// releases any resources allocated during `instantiate`.
    pub cleanup: Option<unsafe extern "C" fn(instance: Handle)>,
}

/// Signature of the `ladspa_descriptor()` entry point a host calls on
/// the loaded `.so`. Corresponds to `LADSPA_Descriptor_Function` in
/// `ladspa.h`.
///
/// A return of NULL signals "no plugin at this index" and terminates
/// the host's enumeration.
pub type DescriptorFn = unsafe extern "C" fn(index: c_ulong) -> *const Descriptor;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    fn port_range_hint_layout_is_packed_three_ints() {
        // ladspa.h declares LADSPA_PortRangeHint as { int; float; float; }.
        // On every supported target this is three 4-byte words with no
        // padding.
        assert_eq!(size_of::<PortRangeHint>(), 12);
        assert_eq!(align_of::<PortRangeHint>(), 4);
    }

    #[test]
    fn descriptor_callback_slots_are_nullable() {
        // The Option<unsafe extern "C" fn(..)> niche makes None == NULL
        // in the C ABI. This is what lets the framework leave
        // run_adding / set_run_adding_gain unset per ADR 0001.
        assert_eq!(
            size_of::<Option<unsafe extern "C" fn(Handle, c_ulong)>>(),
            size_of::<*const c_void>(),
        );
    }
}
