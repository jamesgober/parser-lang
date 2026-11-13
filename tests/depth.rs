//! Deep input must end in a result or a recorded error, never a stack overflow.
//!
//! Infix chains of any associativity are driven from a heap stack, so a long
//! right-associative chain parses at any length. Nesting that goes through
//! `prefix` (parentheses, prefix operators) recurses in the grammar's own code,
//! so the parser bounds it with a depth limit and reports an error past it.

#![allow(clippy::unwrap_used)]

use parser_lang::{Parser, Pratt, Span, Token, TokenKind};

#[derive(Clone, Copy, Debug)]
enum K {
    Num,
    Caret,
    Eq,
    Plus,
    Minus,
    LParen,
    RParen,
    Eof,
}

impl TokenKind for K {
    fn is_eof(&self) -> bool {
        matches!(self, K::Eof)
    }
}

/// Counts operands, so the result is checkable without building a deep value.
struct Count;

impl<'t> Pratt<'t, K> for Count {
    type Output = u64;

    fn prefix(&mut self, p: &mut Parser<'t, K>) -> Option<u64> {
        match p.bump()?.kind() {
            K::Num => Some(1),
            K::Minus => self.expression(p, 100),
            K::LParen => {
                let inner = self.expression(p, 0)?;
                p.expect(|k| matches!(k, K::RParen), "`)`")?;
                Some(inner)
            }
            _ => {
                p.error("expected an operand");
                None
            }
        }
    }

    fn infix_binding(&self, kind: &K) -> Option<(u8, u8)> {
        match kind {
            K::Eq => Some((2, 1)),
            K::Plus => Some((3, 4)),
            K::Caret => Some((6, 5)),
            _ => None,
        }
    }

    fn infix(&mut self, _: &'t Token<K>, left: u64, right: u64) -> Option<u64> {
        Some(left + right)
    }
}

fn tokens(kinds: impl IntoIterator<Item = K>) -> Vec<Token<K>> {
    let mut out: Vec<Token<K>> = kinds
        .into_iter()
        .enumerate()
        .map(|(i, k)| Token::new(k, Span::new(i as u32, i as u32 + 1)))
        .collect();
    let end = out.len() as u32;
    out.push(Token::new(K::Eof, Span::empty(end)));
    out
}

fn chain(op: K, n: usize) -> Vec<Token<K>> {
    tokens(core::iter::once(K::Num).chain((0..n).flat_map(move |_| [op, K::Num])))
}

#[test]
fn million_operator_right_associative_chain_parses() {
    let toks = chain(K::Caret, 1_000_000);
    let mut p = Parser::new(&toks);
    assert_eq!(Count.parse(&mut p), Some(1_000_001));
    assert!(!p.has_errors());
    assert!(p.at_end());
}

#[test]
fn million_operator_assignment_chain_parses() {
    let toks = chain(K::Eq, 1_000_000);
    let mut p = Parser::new(&toks);
    assert_eq!(Count.parse(&mut p), Some(1_000_001));
}

#[test]
fn mixed_long_chain_parses() {
    let ops = [K::Caret, K::Plus, K::Eq];
    let kinds = core::iter::once(K::Num).chain((0..900_000).flat_map(|i| [ops[i % 3], K::Num]));
    let toks = tokens(kinds);
    let mut p = Parser::new(&toks);
    assert_eq!(Count.parse(&mut p), Some(900_001));
}

#[test]
fn deep_parentheses_report_an_error_instead_of_overflowing() {
    let n = 1_000_000;
    let kinds = (0..n)
        .map(|_| K::LParen)
        .chain([K::Num])
        .chain((0..n).map(|_| K::RParen));
    let toks = tokens(kinds);
    let mut p = Parser::new(&toks);
    assert_eq!(Count.parse(&mut p), None);
    assert_eq!(p.errors().len(), 1);
    assert_eq!(p.errors()[0].message(), "expression nested too deeply");
}

#[test]
fn deep_prefix_operators_report_an_error_instead_of_overflowing() {
    let toks = tokens((0..1_000_000).map(|_| K::Minus).chain([K::Num]));
    let mut p = Parser::new(&toks);
    assert_eq!(Count.parse(&mut p), None);
    assert!(p.has_errors());
}

#[test]
fn nesting_at_the_limit_parses_and_one_more_fails() {
    let limit = 64;
    let nest = |n: usize| {
        tokens(
            (0..n)
                .map(|_| K::LParen)
                .chain([K::Num])
                .chain((0..n).map(|_| K::RParen)),
        )
    };
    // The outermost call is depth 1, so `limit - 1` parentheses fit exactly.
    let ok = nest(limit - 1);
    let mut p = Parser::new(&ok).with_depth_limit(limit);
    assert_eq!(p.depth_limit(), limit);
    assert_eq!(Count.parse(&mut p), Some(1));
    assert!(!p.has_errors());

    let deep = nest(limit);
    let mut p = Parser::new(&deep).with_depth_limit(limit);
    assert_eq!(Count.parse(&mut p), None);
    assert_eq!(p.errors().len(), 1);
}

#[test]
fn depth_is_released_after_each_expression() {
    // Many shallow expressions in sequence never accumulate depth.
    let toks = tokens((0..10_000).flat_map(|_| [K::LParen, K::Num, K::RParen]));
    let mut p = Parser::new(&toks).with_depth_limit(3);
    for _ in 0..10_000 {
        assert_eq!(Count.parse(&mut p), Some(1));
    }
    assert!(p.at_end());
    assert!(!p.has_errors());
}

#[test]
fn default_depth_limit_runs_on_a_small_stack() {
    // The default must be safe on a small thread stack in an unoptimised build.
    let limit = Parser::<K>::new(&[]).depth_limit();
    let handle = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            let n = limit - 1;
            let toks = tokens(
                (0..n)
                    .map(|_| K::LParen)
                    .chain([K::Num])
                    .chain((0..n).map(|_| K::RParen)),
            );
            let mut p = Parser::new(&toks);
            Count.parse(&mut p)
        })
        .unwrap();
    assert_eq!(handle.join().unwrap(), Some(1));
}
