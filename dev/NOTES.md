# parser-lang — Engineering Notes

Decisions taken while building parser-lang to 1.0, recorded for review. Lives
alongside `DIRECTIVES.md` and `ROADMAP.md` in `dev/`.

---

## Design decisions

### Output is generic — no `ast-lang` dependency

The master plan lists parser-lang's dependencies as `token, ast, diag`. parser-lang
wires **`token`** (the input stream) and **`diag`** (the diagnostics it records),
but **not `ast`**. A parser's *output* is generic: a grammar's `prefix`/`infix`
functions and its recursive-descent functions return whatever the grammar builds —
an `ast-lang` tree, a value, a count — and parser-lang never names that type. This
is how the established Rust parser libraries (winnow, chumsky, nom) work, and it
keeps parser-lang reusable for any output, AST or not.

So the `ast` relationship is a *use-site* one (a language uses parser-lang to build
an `ast-lang` tree), not an import. Per the build directive's "wire a dependency
only when code actually uses it," `ast-lang` was not added. This mirrors the same
call made in ast-lang (which did not wire `intern`). If a future convenience genuinely
needs ast-lang types, adding the dependency is a non-breaking minor.

### Errors are `diag-lang` diagnostics, collected not thrown

The parser records `diag_lang::Diagnostic`s (severity + message + a labelled span)
and keeps going, so one parse reports many errors in source order. `expect` builds
an `expected …` diagnostic from a caller-supplied description, because a token kind
`K` cannot be named generically (token-lang's `TokenKind` has no display method, and
changing that is a cross-crate contract that was out of scope). The grammar supplies
the human-readable description.

### Kinds matched by predicate, not equality

The cursor matches kinds with `impl FnOnce(&K) -> bool` (`|k| matches!(k,
Kind::Plus)`) rather than `PartialEq`, so a kind that carries data — `Ident(Symbol)`,
`Int(value)` — works without a bound, and matching a category never accidentally
compares the payload.

### Pratt is recursive; expression depth is call-stack bounded

The precedence-climbing driver recurses for the right operand of an operator.
Left-associative chains stay shallow (the loop handles them), but deeply nested or
right-associative input recurses with the nesting depth — the same bound every
recursive-descent parser has. A configurable depth guard is a possible non-breaking
addition; it was left out of the 1.0 surface deliberately. (By contrast, ast-lang's
`walk`/`transform` *are* iterative, because tree size there is unbounded data; here
the bound is source nesting, which is naturally limited.)

### No combinator framework

"Recursive-descent/combinator" is delivered as a cursor with `repeated` /
`separated` helpers for hand-written recursive descent, not a monadic combinator
library (a `Parser` trait with `alt`/`seq`/`map`). That is a much larger, more
opinionated surface to freeze and is better added on top later if wanted.

---

## STATUS — 1.0.0, frozen

parser-lang is built from scaffold through the v0.2.0 core to the v1.0.0 API freeze.
Engineering is complete and green; the commit, tag (`v1.0.0`), push, and
`cargo publish` are left for you, as requested.

**Done:**

- v0.2.0 core — `Parser` cursor (navigation, `expect`, `recover`, `repeated`/
  `separated`, `checkpoint`/`rewind`, collected diagnostics), `Checkpoint`, and the
  `Pratt` precedence engine. `token-lang` and `diag-lang` wired and used; `Token`/
  `TokenKind`/`Span`/`Diagnostic` re-exported.
- v1.0.0 freeze — `docs/API.md` documents the frozen surface and the SemVer promise.

**Quality gates, all green** (Windows local via the rust-lld linker workaround; CI
runs the same on Linux/macOS/Windows, stable + 1.85): `fmt --check`, `clippy
-D warnings` on default / all-features / no-default-features, `test` on default and
all-features, `doc -D warnings`, no-default-features (`no_std`) build, `+1.85`
build, `publish --dry-run`, `deny check`. `#![forbid(unsafe_code)]`, no
`unwrap`/`expect`/`todo!` in shipping code. Counts: 12 unit + 9 calculator + 2
property + 13 doctests.

**For your review before tagging:** the deviation above (no `ast` dependency;
output is generic). It is defensible and documented; flag it if it disagrees with
the master plan's intent. With ast-lang and parser-lang both at 1.0, the front
pipeline (lex → ast → parse) is complete — HQL unblocks here.
