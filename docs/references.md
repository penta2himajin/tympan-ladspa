# References

Reference material consulted during design.

## LADSPA specification

- **`ladspa.h`** (the canonical specification)
  - <https://ladspa.org/ladspa_sdk/ladspa.h.txt>
  - Versions: 1.1 (current, stable since ~2002)
- **LADSPA SDK**
  - <https://ladspa.org/>
  - Includes `ladspa.h`, `applyplugin`, `analyseplugin`, `listplugins`
    utilities
- **LADSPA documentation**
  - <https://ladspa.org/ladspa_sdk/overview.html>
- **Central plugin registry (UniqueID coordination)**
  - <https://ladspa.org/>

## PipeWire integration

- **PipeWire filter-chain module documentation**
  - <https://docs.pipewire.org/page_module_filter_chain.html>
  - Covers LADSPA, LV2, builtin, and sofa filter chains
- **PipeWire 1.0 module documentation index**
  - <https://docs.pipewire.org/page_modules.html>
- **ArchWiki: PipeWire/Examples**
  - <https://wiki.archlinux.org/title/PipeWire/Examples>
  - Practical filter-chain configurations

## Existing LADSPA plugins (reference implementations)

### DeepFilterNet (LADSPA build)

- <https://github.com/Rikorose/DeepFilterNet/tree/main/ladspa>
- Rust-implemented LADSPA plugin for neural noise suppression
- License: MIT / Apache-2.0
- Notable as a contemporary Rust LADSPA reference, though it implements
  LADSPA support directly rather than as a reusable framework

### swh-plugins

- <http://plugin.org.uk/>
- Steve Harris's foundational LADSPA plugin collection (200+ plugins)
- License: GPL-2.0
- Useful as a reference for: port hint conventions, naming patterns,
  numerical stability in DSP code

### CMT (Computer Music Toolkit)

- One of the earliest LADSPA plugin sets
- Provides classic examples of `instantiate`/`run` patterns

### noise-suppression-for-voice

- <https://github.com/werman/noise-suppression-for-voice>
- RNNoise wrapped as a LADSPA plugin
- License: GPL-3.0
- Notable as the most-deployed LADSPA noise suppressor in PipeWire
  filter-chain configurations

## Related Rust crates

### LADSPA-adjacent

- **ladspa** crate (older, semi-archived)
  - <https://crates.io/crates/ladspa>
  - Last updated several years ago; predates current PipeWire patterns
  - Useful as a starting point for naming and structure, but realtime
    safety should not be relied on without auditing

### LV2 (sibling format)

- **lv2** crate
  - <https://crates.io/crates/lv2>
  - Actively maintained Rust framework for the LV2 plugin format
  - Notable as the closest active analogue to `tympan-ladspa`
  - LV2 is more complex (extensions, atoms, URIDs); these features add
    capability but also boilerplate

### Realtime / lock-free

- **crossbeam**
  - <https://crates.io/crates/crossbeam>
  - Lock-free data structures suitable for the audio realtime thread
- **atomic-waker**
  - <https://crates.io/crates/atomic-waker>
  - Cross-thread wake notification (non-blocking)

### General DSP

- **rustfft**: FFT used by many spectral plugins
- **biquad**: Standard biquad filters
- **realfft**: FFT optimized for real-valued signals

## Realtime audio programming background

- **Ross Bencina, "Real-time audio programming 101: time waits for nothing"**
  - <http://www.rossbencina.com/code/real-time-audio-programming-101-time-waits-for-nothing>
  - The canonical introduction to realtime audio constraints
- **Tim Blechmann, "Real-time programming and Linux"**
  - Multiple talks at LAC (Linux Audio Conference)
- **Audio Programmer YouTube channel**
  - Practical realtime audio patterns

## Build and packaging

- **Plugin path conventions**
  - User: `~/.ladspa/` and `~/.config/ladspa/`
  - System: `/usr/lib/ladspa/`, `/usr/local/lib/ladspa/`
  - Host-defined override: `$LADSPA_PATH`
- **PipeWire filter-chain user configuration**
  - `~/.config/pipewire/filter-chain.conf.d/*.conf`
- **Linker visibility for `cdylib`**
  - Use `-Cdefault-symbol-visibility=hidden` and explicit `#[no_mangle]`
    on `ladspa_descriptor` to minimise symbol pollution
