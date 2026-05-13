//! C-callable shims that adapt LADSPA's callback table to the
//! [`Plugin`] trait.
//!
//! Each shim is a `pub unsafe extern "C" fn` parameterised by a
//! [`Plugin`] type. Rust monomorphises one copy per concrete plugin;
//! the [`plugin_entry!`](crate::plugin_entry) macro feeds the
//! appropriate monomorphisations to
//! [`DescriptorBundle::build`](crate::descriptor::DescriptorBundle::build).
//!
//! The shims are the *only* code in the framework that crosses the
//! C ABI boundary. They are unsafe by virtue of accepting raw
//! handles from the host; the safety contract is documented per
//! function.

use core::ffi::c_ulong;

use crate::{plugin::Plugin, port::Ports, raw, realtime::RealtimeContext};

/// Per-instance state owned by the framework. One `Box<Instance<P>>`
/// is allocated per LADSPA `instantiate` call and returned to the
/// host as an opaque [`raw::Handle`].
pub(crate) struct Instance<P: Plugin> {
    plugin: P,
    /// One pointer per port, populated by the host's `connect_port`
    /// calls. Each pointer references a host-owned buffer (audio) or
    /// scalar slot (control). NULL until the host connects the port.
    port_ptrs: Box<[*mut raw::Data]>,
}

impl<P: Plugin> Instance<P> {
    fn new(plugin: P) -> Self {
        let port_count = P::ports().len();
        let port_ptrs = vec![core::ptr::null_mut(); port_count].into_boxed_slice();
        Self { plugin, port_ptrs }
    }
}

/// LADSPA `instantiate` shim.
///
/// # Safety
///
/// `_descriptor` is ignored. The host is responsible for invoking
/// this with a non-null descriptor pointer matching one previously
/// published via `ladspa_descriptor`; the framework does not rely on
/// the value, so no requirement is imposed beyond that.
pub unsafe extern "C" fn instantiate_shim<P: Plugin>(
    _descriptor: *const raw::Descriptor,
    sample_rate: c_ulong,
) -> raw::Handle {
    let sample_rate = sample_rate as u32;
    match P::instantiate(sample_rate) {
        Ok(plugin) => {
            let instance = Box::new(Instance::<P>::new(plugin));
            Box::into_raw(instance) as raw::Handle
        }
        Err(_) => core::ptr::null_mut(),
    }
}

/// LADSPA `connect_port` shim.
///
/// # Safety
///
/// `handle` must be a non-null pointer previously returned by
/// [`instantiate_shim`] for the same `P`. `port` must be in the range
/// `0..P::ports().len()`. `data` must point to a buffer or scalar
/// slot the host will keep alive at least until the next
/// `connect_port` or `cleanup` call.
pub unsafe extern "C" fn connect_port_shim<P: Plugin>(
    handle: raw::Handle,
    port: c_ulong,
    data: *mut raw::Data,
) {
    // SAFETY: contract documented on the function — `handle` is a
    // valid `*mut Instance<P>` and we have exclusive access for the
    // duration of this call (LADSPA hosts serialise lifecycle calls
    // per instance).
    let instance = unsafe { &mut *(handle as *mut Instance<P>) };
    let port = port as usize;
    debug_assert!(
        port < instance.port_ptrs.len(),
        "connect_port: index {port} out of range (port count {})",
        instance.port_ptrs.len(),
    );
    instance.port_ptrs[port] = data;
}

/// LADSPA `activate` shim.
///
/// # Safety
///
/// `handle` must be a non-null pointer previously returned by
/// [`instantiate_shim`] for the same `P`.
pub unsafe extern "C" fn activate_shim<P: Plugin>(handle: raw::Handle) {
    // SAFETY: contract documented on the function.
    let instance = unsafe { &mut *(handle as *mut Instance<P>) };
    instance.plugin.activate();
}

/// LADSPA `run` shim. Executes on the host's realtime audio thread.
///
/// # Safety
///
/// `handle` must be a non-null pointer previously returned by
/// [`instantiate_shim`] for the same `P`. Every port the plugin
/// declared must have been bound via `connect_port` before this is
/// called.
pub unsafe extern "C" fn run_shim<P: Plugin>(handle: raw::Handle, sample_count: c_ulong) {
    // SAFETY: contract documented on the function.
    let instance = unsafe { &mut *(handle as *mut Instance<P>) };
    let frames = sample_count as usize;
    let rt = RealtimeContext::new();
    let mut ports = Ports {
        ptrs: &instance.port_ptrs,
        descriptors: P::ports(),
        frames,
    };
    instance.plugin.run(&rt, frames, &mut ports);
}

/// LADSPA `deactivate` shim.
///
/// # Safety
///
/// `handle` must be a non-null pointer previously returned by
/// [`instantiate_shim`] for the same `P`.
pub unsafe extern "C" fn deactivate_shim<P: Plugin>(handle: raw::Handle) {
    // SAFETY: contract documented on the function.
    let instance = unsafe { &mut *(handle as *mut Instance<P>) };
    instance.plugin.deactivate();
}

/// LADSPA `cleanup` shim. Reclaims the instance.
///
/// # Safety
///
/// `handle` must be a non-null pointer previously returned by
/// [`instantiate_shim`] for the same `P`. After this call returns,
/// the host must not pass `handle` to any further shim.
pub unsafe extern "C" fn cleanup_shim<P: Plugin>(handle: raw::Handle) {
    // SAFETY: reconstructing the `Box` consumes the handle. The
    // function's documented contract makes this the unique
    // reconstruction.
    let instance = unsafe { Box::from_raw(handle as *mut Instance<P>) };
    drop(instance);
}

#[cfg(test)]
mod tests {
    // The test code below dereferences pointers we just received from
    // the FFI shims under test; the safety follows from the test
    // setup rather than from a separately-stated invariant.
    #![allow(clippy::undocumented_unsafe_blocks)]

    use super::*;
    use crate::{
        descriptor::{Callbacks, DescriptorBundle},
        error::InstantiateError,
        port::PortDescriptor,
    };
    use core::sync::atomic::{AtomicUsize, Ordering};

    // A self-contained plugin that records lifecycle events and
    // multiplies the input by the gain control.
    struct Recording;

    static ACTIVATE_COUNT: AtomicUsize = AtomicUsize::new(0);
    static RUN_COUNT: AtomicUsize = AtomicUsize::new(0);
    static DEACTIVATE_COUNT: AtomicUsize = AtomicUsize::new(0);
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    impl Plugin for Recording {
        const UNIQUE_ID: u32 = 0x0001_DEAD;
        const LABEL: &'static str = "recording";
        const NAME: &'static str = "Recording";
        const MAKER: &'static str = "tests";
        const COPYRIGHT: &'static str = "MIT";

        fn ports() -> &'static [PortDescriptor] {
            static PORTS: &[PortDescriptor] = &[
                PortDescriptor::audio_input("In"),
                PortDescriptor::audio_output("Out"),
                PortDescriptor::control_input("Gain"),
            ];
            PORTS
        }

        fn instantiate(_sample_rate: u32) -> Result<Self, InstantiateError> {
            Ok(Self)
        }

        fn activate(&mut self) {
            ACTIVATE_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        fn run(&mut self, _rt: &RealtimeContext, _frames: usize, ports: &mut Ports<'_>) {
            RUN_COUNT.fetch_add(1, Ordering::SeqCst);
            let gain = ports.control_input(2);
            let (input, output) = ports.audio_in_out(0, 1);
            for (i, o) in input.iter().zip(output.iter_mut()) {
                *o = *i * gain;
            }
        }

        fn deactivate(&mut self) {
            DEACTIVATE_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Drop for Recording {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn callbacks() -> Callbacks {
        Callbacks {
            instantiate: instantiate_shim::<Recording>,
            connect_port: connect_port_shim::<Recording>,
            run: run_shim::<Recording>,
            cleanup: cleanup_shim::<Recording>,
            activate: Some(activate_shim::<Recording>),
            deactivate: Some(deactivate_shim::<Recording>),
        }
    }

    #[test]
    fn full_lifecycle_drives_plugin_methods() {
        let bundle = DescriptorBundle::<Recording>::build(callbacks());
        let d = unsafe { &*bundle.descriptor_ptr() };

        // instantiate
        let handle = unsafe { (d.instantiate.unwrap())(d as *const _, 48_000) };
        assert!(!handle.is_null());

        // Connect three buffers, drive run, observe outputs.
        let mut input: [raw::Data; 4] = [0.5, 1.0, 1.5, 2.0];
        let mut output: [raw::Data; 4] = [0.0; 4];
        let mut gain: raw::Data = 2.0;

        unsafe {
            (d.connect_port.unwrap())(handle, 0, input.as_mut_ptr());
            (d.connect_port.unwrap())(handle, 1, output.as_mut_ptr());
            (d.connect_port.unwrap())(handle, 2, &mut gain as *mut _);
        }

        let before_activate = ACTIVATE_COUNT.load(Ordering::SeqCst);
        unsafe { (d.activate.unwrap())(handle) };
        assert_eq!(ACTIVATE_COUNT.load(Ordering::SeqCst), before_activate + 1);

        let before_run = RUN_COUNT.load(Ordering::SeqCst);
        unsafe { (d.run.unwrap())(handle, 4) };
        assert_eq!(RUN_COUNT.load(Ordering::SeqCst), before_run + 1);
        assert_eq!(output, [1.0, 2.0, 3.0, 4.0]);

        let before_deactivate = DEACTIVATE_COUNT.load(Ordering::SeqCst);
        unsafe { (d.deactivate.unwrap())(handle) };
        assert_eq!(
            DEACTIVATE_COUNT.load(Ordering::SeqCst),
            before_deactivate + 1
        );

        let before_drop = DROP_COUNT.load(Ordering::SeqCst);
        unsafe { (d.cleanup.unwrap())(handle) };
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), before_drop + 1);
    }

    #[test]
    fn multi_instance_state_is_per_handle() {
        let bundle = DescriptorBundle::<Recording>::build(callbacks());
        let d = unsafe { &*bundle.descriptor_ptr() };

        // Two independent instances, two independent buffer sets.
        let h1 = unsafe { (d.instantiate.unwrap())(d as *const _, 48_000) };
        let h2 = unsafe { (d.instantiate.unwrap())(d as *const _, 48_000) };
        assert_ne!(h1, h2, "instances must have distinct handles");

        let mut in1: [raw::Data; 2] = [1.0, 2.0];
        let mut out1: [raw::Data; 2] = [0.0; 2];
        let mut g1: raw::Data = 3.0;

        let mut in2: [raw::Data; 2] = [4.0, 5.0];
        let mut out2: [raw::Data; 2] = [0.0; 2];
        let mut g2: raw::Data = 10.0;

        unsafe {
            (d.connect_port.unwrap())(h1, 0, in1.as_mut_ptr());
            (d.connect_port.unwrap())(h1, 1, out1.as_mut_ptr());
            (d.connect_port.unwrap())(h1, 2, &mut g1);
            (d.connect_port.unwrap())(h2, 0, in2.as_mut_ptr());
            (d.connect_port.unwrap())(h2, 1, out2.as_mut_ptr());
            (d.connect_port.unwrap())(h2, 2, &mut g2);

            (d.run.unwrap())(h1, 2);
            (d.run.unwrap())(h2, 2);

            (d.cleanup.unwrap())(h1);
            (d.cleanup.unwrap())(h2);
        }

        assert_eq!(out1, [3.0, 6.0]);
        assert_eq!(out2, [40.0, 50.0]);
    }
}
