# ADR 0003: Plugin authorship via trait impl, not `#[derive(Plugin)]`

- Status: Accepted
- Date: 2026-05-13

## Context

The framework needs a way for users to declare plugin metadata
(`UniqueID`, label, name, maker, copyright, port table) and bind it to
the LADSPA descriptor produced by `ladspa_descriptor()`.

Two surface designs were considered:

1. **Hand-written `impl Plugin for MyPlugin`** with associated `const`
   items and a `ports()` function. Combined with a `plugin_entry!(...)`
   declarative macro that emits the `extern "C" fn ladspa_descriptor`.

2. **`#[derive(Plugin)]` proc-macro** that consumes struct-level
   attributes (`#[plugin(unique_id = 0x1234, label = "...")]`) and field
   attributes for ports, generating both the trait impl and the entry
   point.

## Decision

Start with option 1. Plugin authors write a hand-written trait impl plus
a `plugin_entry!(MyPlugin)` invocation. No proc-macro crate ships in the
initial release.

## Consequences

Positive:

- No `proc-macro2` / `syn` / `quote` dependency. Build times stay fast.
- No second crate (`tympan-ladspa-derive`) to version, document, and
  release in lockstep.
- Error messages on a malformed plugin declaration come from rustc
  pointing at the user's source, not from a proc-macro's `compile_error!`
  emission.
- The trait surface is the canonical specification. A derive macro can
  be added later as syntactic sugar without changing semantics.

Negative:

- Each plugin's source contains a small amount of boilerplate: the trait
  impl header, six `const` declarations, a `ports()` function returning a
  static slice, and the lifecycle methods. This is acceptable for a
  format as small as LADSPA.

## Trigger for reversal

Re-evaluate adding `#[derive(Plugin)]` if, after at least three real
plugins have been written against the trait, the boilerplate is observed
to:

- Account for more than ~25% of any plugin's source file, or
- Cause repeated copy-paste errors across plugins (especially around
  `UNIQUE_ID` collisions or `ports()` shape).

If neither trigger fires, the trait-only design stays.

## References

- `docs/architecture.md` § `Plugin`.
- Open question 3 in `docs/architecture.md`, now resolved by this ADR.
