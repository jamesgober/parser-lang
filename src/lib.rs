//! # parser_lang
//!
//! The parsing toolkit for the `-lang` family: a token cursor a hand-written
//! recursive-descent grammar threads, a Pratt precedence engine for expressions,
//! and error recovery that emits source-annotated diagnostics. It owns no grammar
//! and no AST — the grammar's own functions build whatever output they like — so
//! one toolkit serves every language built on it.
//!
//! ## The pieces
//!
//! - [`Parser`] — a cursor over a [`token_lang`] `&[Token<K>]`. It skips trivia,
//!   stops cleanly at the end of input, matches kinds by predicate (so a kind that
//!   carries data needs no `PartialEq`), and records [`diag_lang`] diagnostics for
//!   recoverable errors so one run reports many. `expect`, `recover`, `checkpoint`,
//!   and the `repeated` / `separated` helpers cover the common shapes.
//! - [`Pratt`] — a precedence-climbing expression engine: describe prefix operands,
//!   infix binding powers, and how to combine them, and the driver handles
//!   precedence and associativity.
//!
//! The grammar consumes tokens from [`token_lang`] and reports errors as
//! [`diag_lang`] [`Diagnostic`]s; what it *produces* is yours — an `ast-lang` tree,
//! a value, anything.
//!
//! ## Quickstart
//!
//! A calculator that evaluates `1 + 2 * 3` to `7` with correct precedence:
//!
//! ```
//! use parser_lang::{Parser, Pratt, Span, Token, TokenKind};
//!
//! #[derive(Clone, Copy)]
//! enum K { Num(i64), Plus, Star, Eof }
//! impl TokenKind for K {
//!     fn is_eof(&self) -> bool { matches!(self, K::Eof) }
//! }
//!
//! struct Calc;
//! impl<'t> Pratt<'t, K> for Calc {
//!     type Output = i64;
//!     fn prefix(&mut self, p: &mut Parser<'t, K>) -> Option<i64> {
//!         match p.bump()?.kind() {
//!             K::Num(n) => Some(*n),
//!             _ => None,
//!         }
//!     }
//!     fn infix_binding(&self, k: &K) -> Option<(u8, u8)> {
//!         match k {
//!             K::Plus => Some((1, 2)),
//!             K::Star => Some((3, 4)),
//!             _ => None,
//!         }
//!     }
//!     fn infix(&mut self, op: &'t Token<K>, l: i64, r: i64) -> Option<i64> {
//!         match op.kind() {
//!             K::Plus => Some(l + r),
//!             K::Star => Some(l * r),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! let s = |i| Span::new(i, i + 1);
//! let tokens = [
//!     Token::new(K::Num(1), s(0)),
//!     Token::new(K::Plus, s(1)),
//!     Token::new(K::Num(2), s(2)),
//!     Token::new(K::Star, s(3)),
//!     Token::new(K::Num(3), s(4)),
//!     Token::new(K::Eof, Span::empty(5)),
//! ];
//! let mut p = Parser::new(&tokens);
//! assert_eq!(Calc.parse(&mut p), Some(7));
//! assert!(!p.has_errors());
//! ```
//!
//! ## Features
//!
//! - `std` (default) — the standard library; without it the crate is `no_std`
//!   (it always needs `alloc`). Forwards to `token-lang/std` and `diag-lang/std`.
//!
//! ## Stability
//!
//! The public surface is being designed across the 0.x series and freezes at
//! `1.0.0`, after which it follows Semantic Versioning: no breaking changes before
//! `2.0`, additions arrive in minor releases, and the MSRV (Rust 1.85) only rises
//! in a minor. The frozen surface is catalogued in
//! [`docs/API.md`](https://github.com/jamesgober/parser-lang/blob/main/docs/API.md).

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

extern crate alloc;

mod parser;
mod pratt;

pub use parser::{Checkpoint, Parser};
pub use pratt::Pratt;

// Re-exported so a grammar can name the token and error types this crate's API is
// built on without also having to depend on `token-lang` and `diag-lang` directly.
pub use diag_lang::Diagnostic;
pub use token_lang::{Span, Token, TokenKind};
