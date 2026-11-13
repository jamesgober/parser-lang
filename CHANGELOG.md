<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>CHANGELOG</b>
</h1>
<p>
  All notable changes to <code>parser-lang</code> will be documented in this file. The format is based on <a href="https://keepachangelog.com/en/1.1.0/">Keep a Changelog</a>,
  and this project adheres to <a href="https://semver.org/spec/v2.0.0.html/">Semantic Versioning</a>.
</p>

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [1.1.0] - 2026-11-12

Deep or hostile input now ends in a result or a recorded error instead of a
stack overflow, and the error log can be capped.

### Added

- `Parser::with_error_limit`, `Parser::error_limit` and
  `Parser::omitted_errors`. Errors past the cap are counted but not stored, so a
  recovering grammar can't use unbounded memory on a large malformed file.
  `Parser::new` still keeps every error.
- `Parser::with_depth_limit` and `Parser::depth_limit`, the nesting limit for
  `Pratt::expression` (default 256).

### Changed

- `Pratt::expression` no longer recurses for infix operators. Pending
  operators wait on a heap stack, so a right-associative chain of any length
  (`a = b = c = ...`, `2 ^ 2 ^ ...`) parses in constant stack space. The driver
  makes the same grammar calls in the same order as before, so results don't
  change. It is also a little faster on long expressions.
- `Pratt::expression` counts its own nesting. Nesting through `prefix`
  (parentheses, prefix operators) past the depth limit records "expression
  nested too deeply" and returns `None`. Before, deep enough input overflowed
  the stack and aborted the process.
- `Parser::has_errors` is also true when errors were recorded past the cap, and
  `Checkpoint` / `rewind` restore the omitted count too.

### Fixed

- A chain of about a million right-associative operators overflowed the stack.
  It now parses.

---

## [1.0.0] - 2026-06-28

API freeze. The cursor and Pratt surface is stable and will not change in a
breaking way before `2.0`. No functional API changes from `0.2.0`.

### Changed

- The public API is declared stable under Semantic Versioning; `docs/API.md`
  catalogues the frozen surface and the SemVer promise.

---

## [0.2.0] - 2026-06-28

The core: the recursive-descent cursor and the Pratt expression engine. `token-lang`
and `diag-lang` are wired and used.

### Added

- `Parser<'t, K>` — a token cursor with trivia skipping, predicate-based matching, error recovery, and collected diagnostics: `peek`/`peek_kind`/`span`/`at_end`/`at`/`bump`/`eat`/`expect`/`error`/`error_at`/`recover`/`repeated`/`separated`/`checkpoint`/`rewind`/`errors`/`has_errors`/`into_errors`.
- `Checkpoint` — an opaque snapshot of the cursor and error log for speculative parsing.
- `Pratt` — a precedence-climbing expression trait (`prefix`/`infix_binding`/`infix`) with a provided `expression`/`parse` driver handling precedence and associativity.
- `Token`, `TokenKind`, `Span` (from `token-lang`) and `Diagnostic` (from `diag-lang`) re-exported from the crate root.
- An end-to-end calculator grammar test (recursive descent + Pratt + recovery + diagnostics) and a property test validating the Pratt engine against an independent shunting-yard oracle; benchmarks for cursor navigation and expression parsing.

### Removed

- The reserved no-op `serde` feature, dropped rather than carried unused toward the frozen 1.0 surface.

---

## [0.1.0] - 2026-06-18

Initial scaffold and repository bootstrap. No domain logic yet &mdash; this release establishes the structure, tooling, and quality gates the implementation will be built on.

### Added

- `Cargo.toml` with crate metadata, Rust 2024 edition, MSRV 1.85.
- Dual `Apache-2.0 OR MIT` license files.
- `README.md`, `CHANGELOG.md`, and a documentation skeleton.
- `REPS.md` compliance baseline.
- `.github/workflows/ci.yml` CI matrix; `deny.toml`, `clippy.toml`, `rustfmt.toml`.
- `dev/DIRECTIVES.md` and `dev/ROADMAP.md` (committed engineering standards + plan).

[Unreleased]: https://github.com/jamesgober/parser-lang/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/jamesgober/parser-lang/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/jamesgober/parser-lang/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/jamesgober/parser-lang/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/parser-lang/releases/tag/v0.1.0
