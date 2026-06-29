# parser-lang &mdash; Engineering Directives

> Engineering standards and the definition of done for this project. Read alongside `REPS.md` (root, authoritative) and `dev/ROADMAP.md` (current phase). If anything here conflicts with `REPS.md`, `REPS.md` wins.

---

## 0. Philosophy

This library is built and maintained to a production standard and treated as a flagship piece of work. Plan the full path, then build one verified step at a time. "Good enough" is treated as a defect. parser-lang is a MAIN-tier foundation crate: every language built on the family parses through it, so its cursor must be total, its precedence correct, and its error recovery dependable on the first try — a parser that panics on malformed input is a defect, not an edge case.

---

## 1. What this is

parser-lang is the parsing toolkit: a cursor over a [`token-lang`] token stream that a hand-written recursive-descent grammar threads, plus a [`Pratt`](Pratt) precedence-climbing engine for expressions, plus error recovery that emits [`diag-lang`] diagnostics and keeps going. It is generic over the language — it owns no grammar and no AST. The grammar's own functions build whatever output they like (an `ast-lang` tree, a value, a count); parser-lang supplies the cursor, the precedence loop, and the error machinery they run on. It owns token navigation, precedence, and recovery only — no lexing (that is `lexer-lang`), no node storage (that is `ast-lang`/`arena-lang`), no rendering (that is `diag-lang`).

The input is `&[Token<K>]` with `K: TokenKind`: the cursor skips trivia and stops at the end-of-input marker, both detected through the `TokenKind` trait. The grammar never matches a kind by equality — it supplies a predicate (`|k| matches!(k, …)`), so a kind that carries data (an interned identifier, a literal value) works without a `PartialEq` bound.

---

## 2. Engineering law (non-negotiable)

- **Performance** — the cursor is allocation-free: navigation is index arithmetic over the borrowed token slice, and lookahead borrows rather than copies. Diagnostics allocate only when an error is actually recorded. No "faster" claim without `criterion` numbers.
- **Correctness** — the Pratt engine respects binding power and associativity exactly: the tree it builds matches a precedence oracle for every operator sequence. Trivia is skipped consistently; the cursor never reads past the end.
- **Robustness** — every cursor operation is total on any token slice, including an empty one, one with no end marker, or a hostile one: out-of-range navigation is a defined no-op, never a panic. Error recovery makes progress — a recovery loop cannot spin without consuming input.
- **Architecture** — SOLID, KISS, YAGNI; one responsibility; the grammar depends on the cursor and the `Pratt` trait, never on a concrete language.
- **Cross-platform** — Linux/macOS/Windows first-class; no platform-specific code.
- **Error handling** — a parse failure is a value: the cursor returns `Option`/`None` and records a diagnostic, never panics. Recoverable syntax errors are collected so one parse reports many.
- **Production-ready** — `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` from the first commit; no stray `println!`/`dbg!`; every public item has rustdoc with a runnable example.

---

## 3. Definition of done

1. Compiles clean on Linux/macOS/Windows, stable and MSRV 1.85.
2. `fmt`, `clippy -D warnings`, `test --all-features`, `cargo doc -D warnings` clean.
3. `cargo audit` + `cargo deny check` pass.
4. No `unwrap`/`expect`/`todo!`/`dbg!` in shipping code.
5. A Tier-1 API exists and headlines the docs.
6. Property tests cover the section-4 invariants.
7. Hot-path (cursor navigation, expression parsing) changes carry benchmarks; no regression over 5%.
8. Docs and `CHANGELOG.md` updated; the matching `docs/release/vX.Y.Z.md` written before the tag.

---

## 4. Project-specific invariants

- The cursor skips trivia: `peek` and `bump` only ever surface non-trivia tokens, and the grammar never sees a token for which `TokenKind::is_trivia` holds.
- Navigation is total: on an empty stream, or once the end is reached, `peek` is `None`, `at_end` is `true`, and `bump` is a no-op — never a panic, never an out-of-range read.
- The Pratt engine is correct: for any sequence of prefix and infix operators, the tree it builds groups by binding power and associativity exactly as a reference precedence climb does.
- Error recovery makes progress: `recover` advances to a synchronizing token or the end, and `repeated`/`separated` cannot loop without consuming input.
- Errors are collected, not thrown: a parse that hits a recoverable error records a `diag-lang` diagnostic and continues, so one run can report several; the diagnostics come back in source order.
- A checkpoint restores the cursor *and* the diagnostics recorded since it was taken, so a speculative parse that is rewound leaves no error behind.
