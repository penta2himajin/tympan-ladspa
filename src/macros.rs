//! The [`plugin_entry!`](crate::plugin_entry) declarative macro.

/// Generate the LADSPA `.so` entry point for a [`Plugin`](crate::Plugin)
/// implementation.
///
/// Place exactly one invocation per `cdylib` crate at module scope:
///
/// ```rust,ignore
/// tympan_ladspa::plugin_entry!(MyPlugin);
/// ```
///
/// The macro emits a `#[no_mangle] pub extern "C" fn ladspa_descriptor`
/// that LADSPA hosts call to enumerate the plugins exported by the
/// shared object. The first call (typically at `.so` load time) lazily
/// constructs a
/// [`DescriptorBundle`](crate::descriptor::DescriptorBundle) inside a
/// process-static `OnceLock`; subsequent calls return the same
/// pointer.
///
/// Because `ladspa_descriptor` is a global C symbol, only one
/// `plugin_entry!` invocation is permitted per `cdylib`. Multi-plugin
/// shared objects are not currently supported; a future macro
/// variant (taking a list of plugin types and dispatching by `index`)
/// will lift that restriction.
#[macro_export]
macro_rules! plugin_entry {
    ($plugin_type:ty) => {
        #[doc(hidden)]
        static __TYMPAN_LADSPA_BUNDLE: ::std::sync::OnceLock<
            $crate::descriptor::DescriptorBundle<$plugin_type>,
        > = ::std::sync::OnceLock::new();

        #[doc(hidden)]
        fn __tympan_ladspa_bundle() -> &'static $crate::descriptor::DescriptorBundle<$plugin_type> {
            __TYMPAN_LADSPA_BUNDLE.get_or_init(|| {
                $crate::descriptor::DescriptorBundle::<$plugin_type>::build(
                    $crate::descriptor::Callbacks {
                        instantiate: $crate::entry::instantiate_shim::<$plugin_type>,
                        connect_port: $crate::entry::connect_port_shim::<$plugin_type>,
                        run: $crate::entry::run_shim::<$plugin_type>,
                        cleanup: $crate::entry::cleanup_shim::<$plugin_type>,
                        activate: ::core::option::Option::Some(
                            $crate::entry::activate_shim::<$plugin_type>,
                        ),
                        deactivate: ::core::option::Option::Some(
                            $crate::entry::deactivate_shim::<$plugin_type>,
                        ),
                    },
                )
            })
        }

        /// LADSPA shared-object entry point. Hosts call this with an
        /// index to enumerate the descriptors this `.so` exposes;
        /// index 0 returns this plugin, every other index returns
        /// NULL (no further plugins).
        ///
        /// # Safety
        ///
        /// Safe to call from any thread; the framework constructs the
        /// descriptor lazily via a `OnceLock` on first call. No
        /// arguments are dereferenced, so the function is in fact
        /// safe — `unsafe` is retained for symmetry with LADSPA's C
        /// signature `LADSPA_Descriptor_Function`.
        #[no_mangle]
        pub unsafe extern "C" fn ladspa_descriptor(
            index: ::core::ffi::c_ulong,
        ) -> *const $crate::raw::Descriptor {
            if index == 0 {
                __tympan_ladspa_bundle().descriptor_ptr()
            } else {
                ::core::ptr::null()
            }
        }
    };
}
