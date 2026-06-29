# parser-lang - Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
> Master plan: ../../_strategy/LANG_COLLECTION.md
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

## v0.1.0 - Scaffold (DONE)
Compiles, CI green, structure correct, no domain logic.
- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt.

## v0.2.0 - Core (DONE)
Recursive-descent cursor + error recovery + Pratt/precedence. HQL unblocks here.
Dependencies wired when first used: `token` (input stream) and `diag` (error
diagnostics). `ast` is not a dependency — output is generic, so the grammar builds
its own AST; the master plan's `ast` listing is a use-site relationship. See
`dev/NOTES.md`.
Exit criteria:
- [x] Every public item has rustdoc + a runnable example.
- [x] Core invariants property-tested (full DIRECTIVES + API authored at this stage).

## v1.0.0 - API freeze (DONE)
Public surface stable and frozen until 2.0.
- [x] docs/API.md marked stable; SemVer promise recorded.
- [x] Full test + benchmark suite green (Windows local + MSRV; CI runs Linux/macOS/Windows on push).
