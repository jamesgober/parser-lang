<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>parser-lang</b>
    <br>
    <sub><sup>PARSER INFRASTRUCTURE</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/parser-lang"><img alt="Crates.io" src="https://img.shields.io/crates/v/parser-lang"></a>
    <a href="https://crates.io/crates/parser-lang"><img alt="Downloads" src="https://img.shields.io/crates/d/parser-lang?color=%230099ff"></a>
    <a href="https://docs.rs/parser-lang"><img alt="docs.rs" src="https://img.shields.io/docsrs/parser-lang"></a>
    <a href="https://github.com/jamesgober/parser-lang/actions"><img alt="CI" src="https://github.com/jamesgober/parser-lang/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        parser-lang is the MAIN-tier crate that follows: Recursive-descent + combinator infra with error recovery and Pratt/precedence. HQL unblocks here. Part of the -lang language-construction family; see _strategy/LANG_COLLECTION.md for the master plan.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition).
    </p>
    <blockquote>
        <strong>Status: stable.</strong> The public API is frozen as of <code>1.0.0</code> and follows Semantic Versioning &mdash; no breaking changes before <code>2.0</code>. See <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

## Installation

```toml
[dependencies]
parser-lang = "1"
```

<br>

## Example

A calculator that evaluates `1 + 2 * 3` to `7` with correct precedence — a Pratt grammar over a `token-lang` stream.

```rust
use parser_lang::{Parser, Pratt, Span, Token, TokenKind};

#[derive(Clone, Copy)]
enum K { Num(i64), Plus, Star, Eof }
impl TokenKind for K {
    fn is_eof(&self) -> bool { matches!(self, K::Eof) }
}

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

let s = |i| Span::new(i, i + 1);
let tokens = [
    Token::new(K::Num(1), s(0)), Token::new(K::Plus, s(1)),
    Token::new(K::Num(2), s(2)), Token::new(K::Star, s(3)),
    Token::new(K::Num(3), s(4)), Token::new(K::Eof, Span::empty(5)),
];
let mut p = Parser::new(&tokens);
assert_eq!(Calc.parse(&mut p), Some(7));
```

<br>

## Status

This is <code>v1.1.0</code>: the public API is stable and frozen under SemVer. The `Parser` cursor (with error recovery) and the `Pratt` precedence engine are complete and catalogued in <a href="./docs/API.md"><code>docs/API.md</code></a>.

<hr>
<br>

## Contributing

See <a href="./dev/DIRECTIVES.md"><code>dev/DIRECTIVES.md</code></a> for engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
