//! Error type returned by fallible plugin lifecycle operations.

/// Reasons [`Plugin::instantiate`](crate::Plugin::instantiate) may
/// refuse to produce an instance.
///
/// `instantiate` is the only lifecycle callback permitted to fail —
/// every other LADSPA callback (`connect_port`, `activate`, `run`,
/// `deactivate`, `cleanup`) must succeed unconditionally. Since
/// `instantiate` runs on the host's setup thread, not the realtime
/// thread, allocation and other non-realtime operations are
/// permitted while constructing this error.
///
/// When `instantiate` returns `Err`, the framework reports the
/// failure to the host by returning a NULL `LADSPA_Handle`. LADSPA's
/// C API does not carry an error code, so the specific variant of
/// `InstantiateError` is observable only in Rust-side tests and via
/// logging.
#[derive(Debug)]
#[non_exhaustive]
pub enum InstantiateError {
    /// The host's requested sample rate falls outside the range this
    /// plugin can process. The payload carries the rate the host
    /// supplied for diagnostic purposes.
    SampleRateUnsupported(u32),

    /// A heap allocation needed during `instantiate` returned an
    /// error. This typically indicates the host process is out of
    /// memory.
    OutOfMemory,

    /// A plugin-specific failure mode. The payload is a short
    /// human-readable description; it must be a `'static` string so
    /// no allocation is required to construct the error.
    Other(&'static str),
}

impl core::fmt::Display for InstantiateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SampleRateUnsupported(rate) => {
                write!(f, "sample rate {rate} Hz is not supported")
            }
            Self::OutOfMemory => f.write_str("out of memory during instantiate"),
            Self::Other(reason) => write!(f, "instantiate failed: {reason}"),
        }
    }
}

impl std::error::Error for InstantiateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_sample_rate() {
        let s = InstantiateError::SampleRateUnsupported(96_000).to_string();
        assert!(s.contains("96000"));
    }

    #[test]
    fn display_formats_other() {
        let s = InstantiateError::Other("missing config").to_string();
        assert!(s.contains("missing config"));
    }

    #[test]
    fn implements_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<InstantiateError>();
    }
}
