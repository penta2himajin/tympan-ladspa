# tympan-gain

A minimal LADSPA gain plugin built on
[`tympan-ladspa`](../../). Multiplies each audio frame by a control-
port gain value.

This crate exists as a worked example and as the framework's primary
end-to-end smoke test. CI builds it as a `cdylib`, verifies the
exported `ladspa_descriptor` symbol with `nm`, reads the plugin
metadata back through `analyseplugin`, and runs a fixture WAV through
the plugin via `applyplugin` (see `.github/workflows/ci.yml`).

## Ports

| Index | Direction | Kind    | Name | Default | Bounds      |
|------:|-----------|---------|------|---------|-------------|
| 0     | input     | audio   | In   | —       | —           |
| 1     | output    | audio   | Out  | —       | —           |
| 2     | input     | control | Gain | 1.0     | [0.0, 4.0]  |

## Build

```sh
cargo build --release -p tympan-gain
# target/release/libtympan_gain.so
```

## Install for PipeWire's filter-chain

```sh
mkdir -p ~/.ladspa
cp target/release/libtympan_gain.so ~/.ladspa/
```

Then reference the plugin in your PipeWire configuration:

```
context.modules = [
  { name = libpipewire-module-filter-chain
    args = {
      node.description = "Tympan gain example"
      filter.graph = {
        nodes = [
          { type = ladspa
            name = gain
            plugin = libtympan_gain
            label = tympan_gain
            control = { Gain = 1.5 }
          }
        ]
      }
      capture.props = { node.name = "input.tympan_gain" node.passive = true }
      playback.props = { node.name = "tympan_gain_source" media.class = Audio/Source }
    }
  }
]
```

## License

Dual-licensed under either of:

- Apache License, Version 2.0 (see [`LICENSE-APACHE`](../../LICENSE-APACHE))
- MIT license (see [`LICENSE-MIT`](../../LICENSE-MIT))

at your option.
