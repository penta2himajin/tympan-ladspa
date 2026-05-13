# tympan-ladspa

[日本語](./README.ja.md)

A Rust framework for writing LADSPA plugins.

`tympan-ladspa` provides Rust abstractions over the LADSPA (Linux Audio
Developer's Simple Plugin API) C interface, enabling Rust applications to
implement audio plugins suitable for hosts such as PipeWire's filter-chain,
Ardour, and other LADSPA-compatible processors.

## Status

**Design phase.** No implementation yet. See [`docs/overview.md`](docs/overview.md)
for planned scope and [`docs/architecture.md`](docs/architecture.md) for the
planned API design.

## Naming

*Tympan* — the tympanal organ of moths, a membrane-based ultrasound sensor
on the abdomen of pyralid and noctuid moths. Evolved to detect the
echolocation calls of bats. The name reflects the library's role: a thin
membrane between the host audio engine and user-space Rust code.

## Development

The project's CI runs `cargo fmt --check`, `cargo clippy --all-targets
-- -D warnings`, and `cargo test` on every PR (see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) and
[ADR 0005](docs/decisions/0005-ci-verification-strategy.md) for the
tiered verification strategy).

To run the same fmt and clippy checks locally before every `git push`,
opt into the repository's pre-push hook:

```sh
git config core.hooksPath .githooks
```

The hook lives in [`.githooks/pre-push`](.githooks/pre-push). It is a
no-op when no `*.rs`, `Cargo.toml`, or `Cargo.lock` files changed in
the pushed range, so documentation-only pushes are not slowed down.
Bypass it for a single push with `git push --no-verify`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

## Documentation

| Doc | Content |
|---|---|
| [`docs/overview.md`](docs/overview.md) | Project purpose, scope, comparison to existing implementations |
| [`docs/architecture.md`](docs/architecture.md) | Planned API design and module layout |
| [`docs/references.md`](docs/references.md) | LADSPA spec, PipeWire integration, prior art |
| [`docs/handoff-protocol.md`](docs/handoff-protocol.md) | Session handoff protocol for long-running work |
