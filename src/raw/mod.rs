//! Low-level FFI declarations matching `ladspa.h` (LADSPA SDK 1.13).
//!
//! This module is the **sole** location where the LADSPA C ABI is
//! declared. Per `CLAUDE.md` § Architectural Boundaries, every higher
//! layer of the framework consumes these definitions; nothing else in
//! the crate may redeclare a LADSPA type or constant.
//!
//! ## Naming
//!
//! Identifiers in `ladspa.h` use a `LADSPA_` prefix because C has no
//! namespaces. Inside this module the prefix is dropped: `Descriptor`
//! corresponds to `LADSPA_Descriptor`, `PORT_INPUT` to
//! `LADSPA_PORT_INPUT`, and so on. Documentation comments link each
//! item back to its upstream name.
//!
//! ## Integer types
//!
//! `ladspa.h` uses bare `int` and `unsigned long`. The mapping in this
//! module follows C semantics on the target platform:
//!
//! | C type          | Rust type       |
//! |-----------------|-----------------|
//! | `int`           | [`c_int`]       |
//! | `unsigned long` | [`c_ulong`]     |
//! | `float`         | [`f32`]         |
//! | `void *`        | `*mut c_void`   |
//!
//! On Linux `c_ulong` is 64 bits for 64-bit targets and 32 bits for
//! 32-bit targets. LADSPA hosts and plugins must agree on this width,
//! which is automatically guaranteed when both sides are built for the
//! same target triple.
//!
//! [`c_int`]: core::ffi::c_int
//! [`c_ulong`]: core::ffi::c_ulong

mod descriptor;
mod types;

pub use descriptor::{Descriptor, DescriptorFn, PortRangeHint};
pub use types::{
    Data, Handle, PortDescriptor, PortRangeHintDescriptor, Properties, HINT_BOUNDED_ABOVE,
    HINT_BOUNDED_BELOW, HINT_DEFAULT_0, HINT_DEFAULT_1, HINT_DEFAULT_100, HINT_DEFAULT_440,
    HINT_DEFAULT_HIGH, HINT_DEFAULT_LOW, HINT_DEFAULT_MASK, HINT_DEFAULT_MAXIMUM,
    HINT_DEFAULT_MIDDLE, HINT_DEFAULT_MINIMUM, HINT_DEFAULT_NONE, HINT_INTEGER, HINT_LOGARITHMIC,
    HINT_SAMPLE_RATE, HINT_TOGGLED, PORT_AUDIO, PORT_CONTROL, PORT_INPUT, PORT_OUTPUT,
    PROPERTY_HARD_RT_CAPABLE, PROPERTY_INPLACE_BROKEN, PROPERTY_REALTIME,
};
