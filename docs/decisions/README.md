# Architecture Decision Records

This directory contains the project's settled architectural decisions.

## Why ADRs

`CLAUDE.md` § SSOT precedence lists ADRs as a top-tier source of truth,
ranked above handoff issues. When the same question recurs across
sessions or PRs, the answer belongs here so it does not get rewritten
each time.

## Lifecycle

- ADRs are numbered sequentially. Filenames follow
  `NNNN-kebab-case-summary.md`.
- An ADR is **Accepted** when merged. Status may later move to
  **Superseded by ADR NNNN** if a follow-up decision overturns it. The
  superseded record stays in the tree for historical context — it is
  not deleted.
- ADRs are short. A single page of context, decision, consequences, and
  (where useful) a documented reversal trigger.

## Index

| ID | Title | Status |
|---:|---|---|
| [0001](0001-skip-run-adding.md) | Skip `run_adding` / `set_run_adding_gain` | Accepted |
| [0002](0002-ports-as-const-slice.md) | Declare ports as `&'static [PortDescriptor]` | Accepted |
| [0003](0003-trait-only-no-derive-macro.md) | Plugin authorship via trait impl, not `#[derive(Plugin)]` | Accepted |
| [0004](0004-no-global-state-multi-instance.md) | Plugin instances are first-class; no global state | Accepted |
