# ADR 0006: Internationalisation policy and file layout

- Status: Accepted
- Date: 2026-05-13

## Context

The project author writes Japanese natively and occasional Japanese
documentation is desirable, but the canonical language for code,
comments, commit messages, design documents, and external
communication (issues, PRs) is English. We need a layout convention
and a scope policy that:

- Keeps a single source of truth so engineering decisions are not
  recorded twice.
- Does not block PRs on translation work.
- Costs as little maintenance as possible during the project's early
  phases.
- Survives a future move to a third language without a structural
  rewrite.

A survey of comparable projects on GitHub examined five layout
patterns. Verified examples and findings:

- **Suffix files** (`README.ja.md`, `docs/foo.ja.md` next to the
  English originals) — used by [WorksApplications/sudachi.rs](https://github.com/WorksApplications/sudachi.rs)
  and [vaaaaanquish/Awesome-Rust-MachineLearning](https://github.com/vaaaaanquish/Awesome-Rust-MachineLearning).
  GitHub renders only `README.md` automatically; the translated file
  is reached via an explicit link near the top.
- **Parallel directories** (`docs/en/`, `docs/ja/`) — rare without a
  static-site generator; closest plain-markdown example is
  [taishi-i/awesome-japanese-nlp-resources](https://github.com/taishi-i/awesome-japanese-nlp-resources/tree/main/docs).
  Forces every existing `docs/...` path to move, breaking `CLAUDE.md`
  references and PR-template links.
- **Locale dirs at repo root** (`en/`, `ja/`) — used by translation-only
  repos such as [qnighy/rust-std-translations](https://github.com/qnighy/rust-std-translations).
  Conflicts with Cargo's expectation that `README.md` lives at the
  crate root; cargo publishes the root README to crates.io.
- **Bucketed translations** (`translations/ja/...`) — used by
  [microsoft/mcp-for-beginners](https://github.com/microsoft/mcp-for-beginners/tree/main/translations).
  Keeps English untouched; adds an extra directory layer to traverse.
- **No in-repo translation** — the dominant pattern in mainstream
  Rust libraries (`tokio`, `clap`, `ripgrep`, `eza`, Rocket).
  Translations live in forks or separate repos.

GitHub itself has no Accept-Language-based locale switch
(confirmed via community discussions
[#31132](https://github.com/orgs/community/discussions/31132),
[#50719](https://github.com/orgs/community/discussions/50719),
[#179316](https://github.com/orgs/community/discussions/179316));
manual switchers placed near the top of the English README are the
universal convention.

## Decision

### 1. Layout: suffix files

Translated documents live next to their English originals with a
`.<lang>.md` suffix. Japanese gets `.ja.md`; future languages follow
[BCP 47](https://www.rfc-editor.org/info/bcp47) tags (`.zh-Hans.md`,
`.de.md`, etc.).

Examples:

```
README.md              # English, authoritative
README.ja.md           # Japanese

docs/overview.md
docs/overview.ja.md
```

The English file is the source of truth in every pair. No directories
named after a language are added to the repository.

### 2. Scope: basic user-facing content only

In scope for Japanese translation:

- `README.md` (project entry point).
- The "user-facing introduction" tier of `docs/` — currently
  `docs/overview.md`. Other documents may be added to this tier later
  by amending this ADR.
- Public-API rustdoc on crate-level items, *when and if* a stable
  public API ships. (Rustdoc tooling renders one language at a time;
  cargo-publish-time selection is out of scope for now.)

Out of scope:

- Engineering-internal documents: `docs/architecture.md`,
  `docs/handoff-protocol.md`, `docs/references.md`.
- Every `docs/decisions/*.md` ADR (including this one).
- `CLAUDE.md` and any other project-instruction files.
- Code comments, doc-comments inside non-crate-level items, commit
  messages, PR descriptions, and issue text.

### 3. Translations never block

A PR that updates an English document is not required to update its
Japanese twin. CI does not enforce parity. Reviewers do not request
ja edits as a blocker. Translation drift is expected and visible by
construction (see § 4).

### 4. Source header in every translated file

The first non-title line of each `.ja.md` file is:

```
> Source: <basename>.md @ <commit-sha-of-source-at-time-of-translation>
```

The header documents which revision of the English source the
translation is derived from. A reader who notices drift can compare
the English source's current `git log` to the recorded SHA. Tooling
to detect drift automatically is *not* required by this ADR; manual
audit before a release is sufficient.

### 5. Language switcher in the English source

Each English file that has a translation pair carries a single line
near the top, immediately under the H1, of the form:

```
[日本語](./<basename>.ja.md)
```

The translated file mirrors a back-link to the English source for the
same reason.

### 6. Adding a new translation pair

The procedure for translating a new document is:

1. Add the new pair to the "Tracked translations" table below in the
   same PR.
2. Create `<name>.ja.md` with the Source header pointing at the
   current HEAD SHA of `<name>.md`.
3. Insert the switcher link into `<name>.md`.

Removing a translation requires removing the entry from the table,
deleting the `.ja.md` file, and removing the switcher link from the
English source.

## Tracked translations

The list grows or shrinks as in § 6. An entry is recorded once the
translated file exists in the repository — placeholders without
content still count.

| English source | Japanese translation | Status |
|---|---|---|
| `README.md` | `README.ja.md` | Placeholder |

## Consequences

Positive:

- Zero migration. `CLAUDE.md`'s `@docs/overview.md`,
  `@docs/architecture.md`, and `@docs/handoff-protocol.md` references
  remain valid. PR templates and ADR cross-links are unaffected.
- `cargo publish` continues to use the root `README.md` without
  configuration.
- A `ls docs/` inspection immediately reveals which files have
  Japanese pairs and which do not — drift is visually obvious.
- The two-language scope avoids the overhead a parallel-directory
  layout would impose.

Negative:

- Suffix files do not auto-render on GitHub the way subdirectory
  `README.md` files do. The switcher link is mandatory for
  discoverability.
- If a third language is added the file count per document doubles
  again. The trigger for revisiting (below) covers this.

## Trigger for revisiting

Re-evaluate this ADR when any of the following holds:

- A third language is requested. At three or more languages the
  suffix scheme starts to clutter `ls` output; reconsider the
  bucketed `translations/<lang>/` layout used by some Microsoft
  projects.
- A static-site generator is adopted (Docusaurus, mdBook with i18n,
  etc.). Their i18n directory conventions supersede the suffix
  scheme.
- A user reports confusion finding the Japanese entry. The switcher
  convention may need amplification (additional badges or
  README front-matter).

## References

- BCP 47 (language tag syntax):
  https://www.rfc-editor.org/info/bcp47
- WorksApplications/sudachi.rs (suffix pattern in a Rust library):
  https://github.com/WorksApplications/sudachi.rs
- microsoft/mcp-for-beginners (translations/ bucket):
  https://github.com/microsoft/mcp-for-beginners/tree/main/translations
- GitHub community discussion confirming no native locale switching:
  https://github.com/orgs/community/discussions/31132
