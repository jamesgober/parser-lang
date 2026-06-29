//! Benchmarks for the two hot paths: cursor navigation over a token stream, and
//! Pratt expression parsing. Both are linear in the token count; these confirm the
//! per-token cost stays flat as input grows.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use parser_lang::{Parser, Pratt, Span, Token, TokenKind};
use std::hint::black_box;

#[derive(Clone, Copy)]
enum Kind {
    Num(i64),
    Plus,
    Star,
    Eof,
}
impl TokenKind for Kind {
    fn is_eof(&self) -> bool {
        matches!(self, Kind::Eof)
    }
}

/// Builds `num + num * num + ...` with `count` numeric operands and an end marker.
fn build(count: usize) -> Vec<Token<Kind>> {
    let mut tokens = Vec::with_capacity(count * 2);
    let mut at = 0u32;
    for i in 0..count {
        tokens.push(Token::new(Kind::Num(i as i64), Span::new(at, at + 1)));
        at += 1;
        if i + 1 < count {
            let op = if i % 2 == 0 { Kind::Plus } else { Kind::Star };
            tokens.push(Token::new(op, Span::new(at, at + 1)));
            at += 1;
        }
    }
    tokens.push(Token::new(Kind::Eof, Span::empty(at)));
    tokens
}

struct Calc;
impl<'t> Pratt<'t, Kind> for Calc {
    type Output = i64;
    fn prefix(&mut self, p: &mut Parser<'t, Kind>) -> Option<i64> {
        match p.bump()?.kind() {
            Kind::Num(n) => Some(*n),
            _ => None,
        }
    }
    fn infix_binding(&self, k: &Kind) -> Option<(u8, u8)> {
        match k {
            Kind::Plus => Some((1, 2)),
            Kind::Star => Some((3, 4)),
            _ => None,
        }
    }
    fn infix(&mut self, op: &'t Token<Kind>, l: i64, r: i64) -> Option<i64> {
        match op.kind() {
            Kind::Plus => Some(l.wrapping_add(r)),
            Kind::Star => Some(l.wrapping_mul(r)),
            _ => None,
        }
    }
}

fn bench_cursor(c: &mut Criterion) {
    let mut group = c.benchmark_group("cursor");
    for &count in &[64usize, 1024, 16384] {
        let tokens = build(count);
        group.bench_with_input(
            BenchmarkId::from_parameter(tokens.len()),
            &tokens,
            |b, tokens| {
                b.iter(|| {
                    let mut p = Parser::new(black_box(tokens));
                    let mut n = 0u64;
                    while p.bump().is_some() {
                        n += 1;
                    }
                    n
                });
            },
        );
    }
    group.finish();
}

fn bench_pratt(c: &mut Criterion) {
    let mut group = c.benchmark_group("pratt");
    for &count in &[64usize, 1024, 16384] {
        let tokens = build(count);
        group.bench_with_input(
            BenchmarkId::from_parameter(tokens.len()),
            &tokens,
            |b, tokens| {
                b.iter(|| {
                    let mut p = Parser::new(black_box(tokens));
                    Calc.parse(&mut p)
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_cursor, bench_pratt);
criterion_main!(benches);
