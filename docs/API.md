# parser-lang &mdash; API Reference

> Complete reference for every public item in `parser-lang`, with examples.
> **Status: stable as of `1.0.0`.** The surface below is frozen under Semantic Versioning — no breaking changes before `2.0`.

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [`Parser`](#parser)
- [`Checkpoint`](#checkpoint)
- [`Pratt`](#pratt)
- [Re-exports](#re-exports)
- [Feature flags](#feature-flags)
- [Frozen surface](#frozen-surface)

---

## Overview

parser-lang is the parsing toolkit of the `-lang` family: a token cursor a
hand-written recursive-descent grammar threads, a Pratt precedence engine for
expressions, and error recovery that records source-annotated diagnostics. It owns
no grammar and no AST — the grammar's own functions build whatever output they
like — so one toolkit serves every language.

Input is a `&[Token<K>]` from [`token-lang`](https://docs.rs/token-lang); errors
are [`diag-lang`](https://docs.rs/diag-lang) `Diagnostic`s. Kinds are matched by
predicate (`|k| matches!(k, Kind::Plus)`), so a kind that carries data — an
interned identifier, a literal — works without a `PartialEq` bound.

---

## Installation

```toml
[dependencies]
parser-lang = "1"
```

---

## `Parser`

`Parser<'t, K>` is the cursor a recursive-descent grammar drives. It borrows the
token stream, skips trivia automatically, stops cleanly at the end of input, and
collects diagnostics so one parse reports many errors.

```rust
use parser_lang::{Parser, Span, Token, TokenKind};

#[derive(Clone, Copy)]
enum K { Num, Plus, Eof }
impl TokenKind for K { fn is_eof(&self) -> bool { matches!(self, K::Eof) } }

let tokens = [
    Token::new(K::Num, Span::new(0, 1)),
    Token::new(K::Plus, Span::new(2, 3)),
    Token::new(K::Num, Span::new(4, 5)),
    Token::new(K::Eof, Span::empty(5)),
];
let mut p = Parser::new(&tokens);
assert!(p.at(|k| matches!(k, K::Num)));
p.bump();
assert!(p.eat(|k| matches!(k, K::Plus)).is_some());
```

| Method | Description |
|--------|-------------|
| `new(tokens: &'t [Token<K>]) -> Parser<'t, K>` | A cursor over `tokens`, at the first significant token. |
| `with_error_limit(self, limit: usize) -> Parser<'t, K>` _(1.1)_ | Store at most `limit` errors; later ones are only counted. |
| `error_limit(&self) -> Option<usize>` / `omitted_errors(&self) -> usize` _(1.1)_ | The error cap, and how many errors went past it. |
| `with_depth_limit(self, limit: usize) -> Parser<'t, K>` / `depth_limit(&self) -> usize` _(1.1)_ | How deeply `Pratt::expression` calls may nest (default 256). |
| `peek(&self) -> Option<&'t Token<K>>` | The current significant token, or `None` at the end. |
| `peek_kind(&self) -> Option<&'t K>` | The current token's kind. |
| `span(&self) -> Span` | The current token's span, or an empty end-of-input span. |
| `at_end(&self) -> bool` | `true` at end of input or on the eof marker. |
| `at(&self, pred) -> bool` | Whether the current kind satisfies `pred`. |
| `bump(&mut self) -> Option<&'t Token<K>>` | Consume and return the current token. |
| `eat(&mut self, pred) -> Option<&'t Token<K>>` | Consume the current token if it matches `pred`. |
| `expect(&mut self, pred, description: &str) -> Option<&'t Token<K>>` | Consume if it matches, else record `expected {description}`. |
| `error(&mut self, message)` / `error_at(&mut self, span, message)` | Record an error diagnostic. |
| `recover(&mut self, sync)` | Skip to a synchronizing token (or the end) for recovery. |
| `repeated(&mut self, parse) -> Vec<T>` | Parse items until `parse` returns `None`. |
| `separated(&mut self, sep, parse) -> Vec<T>` | Parse a `sep`-separated list. |
| `checkpoint(&self) -> Checkpoint` / `rewind(&mut self, cp)` | Save and restore the cursor and error log, for speculative parsing. |
| `errors(&self) -> &[Diagnostic]` / `has_errors(&self) -> bool` / `into_errors(self) -> Vec<Diagnostic>` | Inspect or take the recorded diagnostics. `has_errors` also counts errors past the cap. |

The predicate methods take `impl FnOnce(&K) -> bool`; `recover`/`separated` take
`Fn`/`FnMut`. The token references returned are tied to the stream's lifetime
`'t`, so they outlive the borrow of the parser.

**Limits.** A recovering grammar can record one error per token, so on untrusted
input set a cap with `with_error_limit`; errors past it are counted in
`omitted_errors` but not stored. `Parser::new` keeps every error, as in 1.0. The
depth limit bounds nesting through `Pratt::prefix` (see `Pratt` below).

## `Checkpoint`

An opaque, `Copy` snapshot of the cursor position and error counts, from
`Parser::checkpoint`. Pass it to `Parser::rewind` to undo a speculative parse —
the cursor returns and any diagnostics recorded since are dropped.

---

## `Pratt`

A precedence-climbing expression grammar, driven over a `Parser`. Implement three
methods; the provided `expression` / `parse` driver handles precedence and
associativity.

| Method | Description |
|--------|-------------|
| `type Output` | What the grammar builds (a node, a value, …). |
| `prefix(&mut self, p) -> Option<Output>` | Parse an operand: literal, parenthesized group, prefix-unary, or a postfix chain. |
| `infix_binding(&self, kind: &K) -> Option<(u8, u8)>` | The `(left, right)` binding power of an infix operator, or `None`. Left below right is left-associative; above is right-associative. |
| `infix(&mut self, op, left, right) -> Option<Output>` | Combine operands with the operator. |
| `expression(&mut self, p, min_bp) -> Option<Output>` _(provided)_ | The precedence-climbing driver. |
| `parse(&mut self, p) -> Option<Output>` _(provided)_ | Parse a full expression (minimum binding power 0). |

Returning `None` from any method signals a recoverable error (after recording a
diagnostic on the parser), and the driver unwinds.

**Depth.** The driver never recurses for infix operators: pending operators
wait on a heap stack, so a chain of any length and associativity (`a = b = c =
...`, `2 ^ 2 ^ ...`) parses in constant call-stack space. Nesting through
`prefix` (parentheses, prefix operators that call `expression`) does use the
call stack, one level per call, so `expression` checks the parser's depth limit
(default 256, set with `Parser::with_depth_limit`). Past it, it records
"expression nested too deeply" and returns `None`. Postfix operators (a call `()`,
an index `[]`) are handled inside `prefix` by looping on the trailing tokens after
the atom.

```rust
use parser_lang::{Parser, Pratt, Span, Token, TokenKind};

#[derive(Clone, Copy)]
enum K { Num(i64), Plus, Star, Eof }
impl TokenKind for K { fn is_eof(&self) -> bool { matches!(self, K::Eof) } }

struct Calc;
impl<'t> Pratt<'t, K> for Calc {
    type Output = i64;
    fn prefix(&mut self, p: &mut Parser<'t, K>) -> Option<i64> {
        match p.bump()?.kind() { K::Num(n) => Some(*n), _ => None }
    }
    fn infix_binding(&self, k: &K) -> Option<(u8, u8)> {
        match k { K::Plus => Some((1, 2)), K::Star => Some((3, 4)), _ => None }
    }
    fn infix(&mut self, op: &'t Token<K>, l: i64, r: i64) -> Option<i64> {
        match op.kind() { K::Plus => Some(l + r), K::Star => Some(l * r), _ => None }
    }
}
```

---

## Re-exports

The token and error types this crate's API is built on are re-exported, so a
grammar need not also name `token-lang` and `diag-lang`:

- `Token`, `TokenKind`, `Span` — from [`token-lang`](https://docs.rs/token-lang); the input stream.
- `Diagnostic` — from [`diag-lang`](https://docs.rs/diag-lang); the recorded errors.

---

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | yes | Standard library. With it off, the crate is `no_std` (it always needs `alloc`). Forwards to `token-lang/std` and `diag-lang/std`. |

---

## Frozen surface

The items above are the complete public API, frozen as of `1.0.0`, under Semantic
Versioning: no breaking change before `2.0`, additions in minors, MSRV (Rust 1.85)
rising only in a minor.

Deliberately left out of 1.0, each addable later without a breaking change:

- **No `ast-lang` dependency.** A parser's output is generic — the grammar's
  actions build whatever they like (an `ast-lang` tree, a value), so parser-lang
  needs only the token and diagnostic crates. The master plan lists `ast` as a
  dependency; it is a use-site relationship, not an import. See `dev/NOTES.md`.
- **No combinator trait / monadic layer.** parser-lang is a cursor-and-helpers
  toolkit for hand-written recursive descent (with `repeated` / `separated`), not a
  combinator framework; a combinator layer could be added on top.
- **Depth of hand-written recursion.** Since 1.1 the Pratt driver bounds its
  own nesting (see `Pratt`). Recursion in the grammar's other functions, such
  as nested statements or blocks, is the grammar's to bound.

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
