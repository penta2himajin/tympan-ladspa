//! Construction of the LADSPA descriptor table from a [`Plugin`]
//! implementation.
//!
//! This module is invoked once per shared object load, by the
//! `ladspa_descriptor` function the [`plugin_entry!`](crate::plugin_entry)
//! macro emits. The resulting [`DescriptorBundle`] owns the
//! nul-terminated strings, port arrays, and the [`raw::Descriptor`]
//! itself; the bundle lives inside a `static OnceLock` for the
//! lifetime of the loaded `.so`.
//!
//! The bundle's contents are immutable after `build`, so its raw
//! pointers remain valid for as long as the bundle does — i.e. for
//! the entire program lifetime once the `.so` has been mapped.

use core::ffi::{c_char, c_ulong};
use core::marker::PhantomData;
use std::ffi::CString;

use crate::{
    plugin::Plugin,
    port::PortDescriptor,
    raw::{self, Data, Handle},
};

/// Owned LADSPA descriptor table for a single [`Plugin`] type.
///
/// `DescriptorBundle` allocates and pins the strings and port arrays
/// the LADSPA descriptor references. The bundle is one-shot: built
/// once on first call to `ladspa_descriptor`, stored in a
/// process-global `OnceLock`, and never mutated.
pub struct DescriptorBundle<P: Plugin> {
    // Field order matters for drop ordering. The `descriptor` and
    // pointer arrays go before the strings they reference so that
    // freeing the pointer-holding storage happens before freeing the
    // pointee strings. In practice the bundle is never dropped
    // (it lives inside a `static OnceLock`), but the conservative
    // ordering keeps Miri and any future tests happy.
    /// The raw LADSPA descriptor exposed to hosts. Its pointer
    /// fields refer into the surrounding bundle fields.
    descriptor: raw::Descriptor,

    /// Array of bitfields, one per port, indexed by port position.
    /// Each entry combines a direction (`PORT_INPUT`/`PORT_OUTPUT`)
    /// and a kind (`PORT_AUDIO`/`PORT_CONTROL`) bit.
    _port_descriptors: Box<[raw::PortDescriptor]>,

    /// Array of `*const c_char` referencing into `port_name_strings`.
    _port_names: Box<[*const c_char]>,

    /// Array of LADSPA port range hints.
    _port_range_hints: Box<[raw::PortRangeHint]>,

    /// Owning storage for port names (one `CString` per port).
    _port_name_strings: Box<[CString]>,

    /// Owning storage for the plugin's metadata strings.
    _label: CString,
    _name: CString,
    _maker: CString,
    _copyright: CString,

    _phantom: PhantomData<fn() -> P>,
}

// SAFETY: After `build` returns, the bundle is never mutated. Every
// pointer field of `descriptor` either points into a `Box` field of
// the same bundle (with the same lifetime) or to a process-static
// function. The only multi-threaded access is by LADSPA hosts
// dereferencing the descriptor, which is a read-only operation. The
// `*const c_char` fields that make `raw::Descriptor` `!Sync` by
// default are sound to share given that immutability. The bundle
// itself lives in a `static OnceLock`, which already requires `Sync`
// on its inner type — this `unsafe impl` is what unlocks that.
unsafe impl<P: Plugin> Sync for DescriptorBundle<P> {}
// SAFETY: same reasoning — the bundle holds only owned and shared
// pointers to immutable data, all of which are transferable across
// threads.
unsafe impl<P: Plugin> Send for DescriptorBundle<P> {}

/// Function pointers the framework's FFI shim exports for a single
/// `Plugin` type. Kept out of the public surface; the
/// [`plugin_entry!`](crate::plugin_entry) macro wires this up.
#[doc(hidden)]
pub struct Callbacks {
    /// Mandatory: construct a plugin instance.
    pub instantiate: unsafe extern "C" fn(*const raw::Descriptor, c_ulong) -> Handle,
    /// Mandatory: bind a port to a host buffer.
    pub connect_port: unsafe extern "C" fn(Handle, c_ulong, *mut Data),
    /// Mandatory: process audio.
    pub run: unsafe extern "C" fn(Handle, c_ulong),
    /// Mandatory: destroy an instance.
    pub cleanup: unsafe extern "C" fn(Handle),
    /// Optional: reset state before processing.
    pub activate: Option<unsafe extern "C" fn(Handle)>,
    /// Optional: counterpart to `activate`.
    pub deactivate: Option<unsafe extern "C" fn(Handle)>,
}

impl<P: Plugin> DescriptorBundle<P> {
    /// Construct the bundle.
    ///
    /// Called exactly once per shared object load, inside the
    /// `ladspa_descriptor` shim emitted by
    /// [`plugin_entry!`](crate::plugin_entry).
    ///
    /// # Panics
    ///
    /// Panics if any of the plugin's metadata strings (`LABEL`,
    /// `NAME`, `MAKER`, `COPYRIGHT`, or a port name) contains an
    /// embedded NUL byte. Authors are responsible for ensuring their
    /// metadata is well-formed; this is a programmer error, not a
    /// runtime condition.
    pub fn build(callbacks: Callbacks) -> Self {
        let label = make_cstring("LABEL", P::LABEL);
        let name = make_cstring("NAME", P::NAME);
        let maker = make_cstring("MAKER", P::MAKER);
        let copyright = make_cstring("COPYRIGHT", P::COPYRIGHT);

        let ports: &'static [PortDescriptor] = P::ports();
        let port_count = ports.len();

        let port_descriptors: Box<[raw::PortDescriptor]> =
            ports.iter().map(|p| p.kind().ladspa_bits()).collect();

        let port_name_strings: Box<[CString]> = ports
            .iter()
            .map(|p| make_cstring("port name", p.name()))
            .collect();
        let port_names: Box<[*const c_char]> =
            port_name_strings.iter().map(|s| s.as_ptr()).collect();

        let port_range_hints: Box<[raw::PortRangeHint]> = ports
            .iter()
            .map(|p| {
                let h = p.hints();
                raw::PortRangeHint {
                    hint_descriptor: h.descriptor,
                    lower_bound: h.lower,
                    upper_bound: h.upper,
                }
            })
            .collect();

        let descriptor = raw::Descriptor {
            unique_id: P::UNIQUE_ID as c_ulong,
            label: label.as_ptr(),
            properties: P::PROPERTIES,
            name: name.as_ptr(),
            maker: maker.as_ptr(),
            copyright: copyright.as_ptr(),
            port_count: port_count as c_ulong,
            port_descriptors: port_descriptors.as_ptr(),
            port_names: port_names.as_ptr(),
            port_range_hints: port_range_hints.as_ptr(),
            implementation_data: core::ptr::null_mut(),
            instantiate: Some(callbacks.instantiate),
            connect_port: Some(callbacks.connect_port),
            activate: callbacks.activate,
            run: Some(callbacks.run),
            // ADR 0001: framework leaves run_adding /
            // set_run_adding_gain unset; hosts fall back to `run`.
            run_adding: None,
            set_run_adding_gain: None,
            deactivate: callbacks.deactivate,
            cleanup: Some(callbacks.cleanup),
        };

        Self {
            descriptor,
            _port_descriptors: port_descriptors,
            _port_names: port_names,
            _port_range_hints: port_range_hints,
            _port_name_strings: port_name_strings,
            _label: label,
            _name: name,
            _maker: maker,
            _copyright: copyright,
            _phantom: PhantomData,
        }
    }

    /// Pointer to the LADSPA descriptor owned by this bundle.
    ///
    /// Valid for as long as the bundle is alive — which, in normal
    /// operation, is for the entire lifetime of the loaded `.so`.
    pub fn descriptor_ptr(&self) -> *const raw::Descriptor {
        &self.descriptor as *const _
    }
}

fn make_cstring(field: &str, value: &str) -> CString {
    CString::new(value)
        .unwrap_or_else(|_| panic!("Plugin::{field} {value:?} contains an embedded NUL byte"))
}

#[cfg(test)]
mod tests {
    // The test code below dereferences pointers we just published
    // through the LADSPA descriptor we constructed in the same test;
    // the safety follows from the test setup rather than from a
    // separately-stated invariant.
    #![allow(clippy::undocumented_unsafe_blocks)]

    use super::*;
    use crate::{
        port::{PortDefault, PortDescriptor, Ports},
        InstantiateError,
    };
    use core::ffi::CStr;

    struct TestPlugin;

    impl Plugin for TestPlugin {
        const UNIQUE_ID: u32 = 0x0001_BEEF;
        const LABEL: &'static str = "test_gain";
        const NAME: &'static str = "Test Gain";
        const MAKER: &'static str = "tympan-ladspa tests";
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

        fn run(
            &mut self,
            _rt: &crate::realtime::RealtimeContext,
            _frames: usize,
            _ports: &mut Ports<'_>,
        ) {
        }
    }

    unsafe extern "C" fn dummy_instantiate(_d: *const raw::Descriptor, _r: c_ulong) -> raw::Handle {
        core::ptr::null_mut()
    }
    unsafe extern "C" fn dummy_connect(_h: raw::Handle, _p: c_ulong, _d: *mut Data) {}
    unsafe extern "C" fn dummy_run(_h: raw::Handle, _n: c_ulong) {}
    unsafe extern "C" fn dummy_cleanup(_h: raw::Handle) {}
    unsafe extern "C" fn dummy_activate(_h: raw::Handle) {}

    fn make_bundle() -> DescriptorBundle<TestPlugin> {
        DescriptorBundle::<TestPlugin>::build(Callbacks {
            instantiate: dummy_instantiate,
            connect_port: dummy_connect,
            run: dummy_run,
            cleanup: dummy_cleanup,
            activate: Some(dummy_activate),
            deactivate: None,
        })
    }

    #[test]
    fn descriptor_carries_plugin_metadata() {
        let bundle = make_bundle();
        let d = unsafe { &*bundle.descriptor_ptr() };

        assert_eq!(d.unique_id as u32, TestPlugin::UNIQUE_ID);
        assert_eq!(d.port_count, 3);

        let label = unsafe { CStr::from_ptr(d.label) };
        assert_eq!(label.to_str().unwrap(), TestPlugin::LABEL);

        let name = unsafe { CStr::from_ptr(d.name) };
        assert_eq!(name.to_str().unwrap(), TestPlugin::NAME);

        let maker = unsafe { CStr::from_ptr(d.maker) };
        assert_eq!(maker.to_str().unwrap(), TestPlugin::MAKER);

        let copyright = unsafe { CStr::from_ptr(d.copyright) };
        assert_eq!(copyright.to_str().unwrap(), TestPlugin::COPYRIGHT);
    }

    #[test]
    fn port_arrays_match_ports_method() {
        let bundle = make_bundle();
        let d = unsafe { &*bundle.descriptor_ptr() };

        let descs =
            unsafe { core::slice::from_raw_parts(d.port_descriptors, d.port_count as usize) };
        assert_eq!(descs[0], raw::PORT_AUDIO | raw::PORT_INPUT);
        assert_eq!(descs[1], raw::PORT_AUDIO | raw::PORT_OUTPUT);
        assert_eq!(descs[2], raw::PORT_CONTROL | raw::PORT_INPUT);

        let names = unsafe { core::slice::from_raw_parts(d.port_names, d.port_count as usize) };
        let n0 = unsafe { CStr::from_ptr(names[0]) };
        assert_eq!(n0.to_str().unwrap(), "In");
        let n2 = unsafe { CStr::from_ptr(names[2]) };
        assert_eq!(n2.to_str().unwrap(), "Gain");

        let hints =
            unsafe { core::slice::from_raw_parts(d.port_range_hints, d.port_count as usize) };
        // Audio ports carry no hints in this test plugin.
        assert_eq!(hints[0].hint_descriptor, 0);
        // Control port carries the default + bounds bits.
        let gain_hint = hints[2].hint_descriptor;
        assert_eq!(gain_hint & raw::HINT_DEFAULT_MASK, raw::HINT_DEFAULT_1);
        assert!(gain_hint & raw::HINT_BOUNDED_BELOW != 0);
        assert!(gain_hint & raw::HINT_BOUNDED_ABOVE != 0);
        assert_eq!(hints[2].lower_bound, 0.0);
        assert_eq!(hints[2].upper_bound, 4.0);
    }

    #[test]
    fn run_adding_callbacks_remain_none() {
        let bundle = make_bundle();
        let d = unsafe { &*bundle.descriptor_ptr() };
        assert!(d.run_adding.is_none());
        assert!(d.set_run_adding_gain.is_none());
    }

    #[test]
    fn optional_callbacks_threaded_through() {
        let bundle = make_bundle();
        let d = unsafe { &*bundle.descriptor_ptr() };
        // activate was provided, deactivate was not.
        assert!(d.activate.is_some());
        assert!(d.deactivate.is_none());
    }
}
