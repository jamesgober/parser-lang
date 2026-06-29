//! An end-to-end calculator grammar exercising the whole toolkit: a Pratt
//! expression engine with parentheses and prefix negation, a recursive-descent
//! statement loop, error recovery to a synchronizing token, and diagnostics.

#![allow(clippy::unwrap_used)]

use parser_lang::{Diagnostic, Parser, Pratt, Span, Token, TokenKind};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Num(i64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Semi,
    Ws,
    Eof,
}

impl TokenKind for Kind {
    fn is_trivia(&self) -> bool {
        matches!(self, Kind::Ws)
    }
    fn is_eof(&self) -> bool {
        matches!(self, Kind::Eof)
    }
}

/// Builds a token stream, assigning each token a one-column span by index and
/// appending an end-of-input marker.
fn lex(kinds: &[Kind]) -> Vec<Token<Kind>> {
    let mut tokens: Vec<Token<Kind>> = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| Token::new(*k, Span::new(i as u32, i as u32 + 1)))
        .collect();
    let end = kinds.len() as u32;
    tokens.push(Token::new(Kind::Eof, Span::empty(end)));
    tokens
}

/// The calculator grammar: evaluates an expression to an `i64` as it parses.
struct Calc;

impl<'t> Pratt<'t, Kind> for Calc {
    type Output = i64;

    fn prefix(&mut self, p: &mut Parser<'t, Kind>) -> Option<i64> {
        match p.peek_kind()? {
            Kind::Num(_) => match p.bump()?.kind() {
                Kind::Num(n) => Some(*n),
                _ => None,
            },
            Kind::Minus => {
                p.bump();
                let operand = self.expression(p, 100)?; // prefix binds tighter than infix
                Some(-operand)
            }
            Kind::LParen => {
                p.bump();
                let inner = self.parse(p)?;
                p.expect(|k| matches!(k, Kind::RParen), "`)`")?;
                Some(inner)
            }
            _ => {
                p.error("expected an expression");
                None
            }
        }
    }

    fn infix_binding(&self, kind: &Kind) -> Option<(u8, u8)> {
        match kind {
            Kind::Plus | Kind::Minus => Some((1, 2)),
            Kind::Star | Kind::Slash => Some((3, 4)),
            _ => None,
        }
    }

    fn infix(&mut self, op: &'t Token<Kind>, left: i64, right: i64) -> Option<i64> {
        match op.kind() {
            Kind::Plus => Some(left + right),
            Kind::Minus => Some(left - right),
            Kind::Star => Some(left * right),
            Kind::Slash => left.checked_div(right), // None on divide-by-zero
            _ => None,
        }
    }
}

/// Parses a single expression and returns its value, ignoring any trailing tokens.
fn eval(kinds: &[Kind]) -> Option<i64> {
    let tokens = lex(kinds);
    let mut p = Parser::new(&tokens);
    Calc.parse(&mut p)
}

#[test]
fn test_multiplication_binds_tighter_than_addition() {
    // 1 + 2 * 3 = 7
    assert_eq!(
        eval(&[
            Kind::Num(1),
            Kind::Plus,
            Kind::Num(2),
            Kind::Star,
            Kind::Num(3)
        ]),
        Some(7)
    );
}

#[test]
fn test_parentheses_override_precedence() {
    // (1 + 2) * 3 = 9
    assert_eq!(
        eval(&[
            Kind::LParen,
            Kind::Num(1),
            Kind::Plus,
            Kind::Num(2),
            Kind::RParen,
            Kind::Star,
            Kind::Num(3),
        ]),
        Some(9)
    );
}

#[test]
fn test_subtraction_is_left_associative() {
    // 10 - 2 - 3 = 5  (not 11)
    assert_eq!(
        eval(&[
            Kind::Num(10),
            Kind::Minus,
            Kind::Num(2),
            Kind::Minus,
            Kind::Num(3)
        ]),
        Some(5)
    );
}

#[test]
fn test_prefix_negation() {
    // -2 * 3 = -6
    assert_eq!(
        eval(&[Kind::Minus, Kind::Num(2), Kind::Star, Kind::Num(3)]),
        Some(-6)
    );
}

#[test]
fn test_whitespace_is_skipped() {
    // 1 <ws> + <ws> 2
    assert_eq!(
        eval(&[Kind::Num(1), Kind::Ws, Kind::Plus, Kind::Ws, Kind::Num(2)]),
        Some(3)
    );
}

#[test]
fn test_missing_operand_records_a_diagnostic() {
    // 1 +   (nothing after the operator)
    let tokens = lex(&[Kind::Num(1), Kind::Plus]);
    let mut p = Parser::new(&tokens);
    let result = Calc.parse(&mut p);
    assert_eq!(result, None);
    assert!(p.has_errors());
    let diag: &Diagnostic = &p.errors()[0];
    assert_eq!(diag.message(), "expected an expression");
}

#[test]
fn test_unclosed_paren_records_expected_rparen() {
    // ( 1 + 2     (no closing paren; hits eof)
    let tokens = lex(&[Kind::LParen, Kind::Num(1), Kind::Plus, Kind::Num(2)]);
    let mut p = Parser::new(&tokens);
    let _ = Calc.parse(&mut p);
    assert!(p.has_errors());
    assert_eq!(p.errors()[0].message(), "expected `)`");
}

/// A statement loop: each statement is an expression terminated by `;`. On a
/// syntax error it recovers to the next `;` and keeps going, so one bad statement
/// does not lose the rest.
fn parse_program(p: &mut Parser<'_, Kind>) -> Vec<i64> {
    let mut values = Vec::new();
    while !p.at_end() {
        match Calc.parse(p) {
            Some(value) if p.expect(|k| matches!(k, Kind::Semi), "`;`").is_some() => {
                values.push(value);
            }
            _ => {
                // Recover: skip to the next `;`, consume it, and continue.
                p.recover(|k| matches!(k, Kind::Semi));
                p.eat(|k| matches!(k, Kind::Semi));
            }
        }
    }
    values
}

#[test]
fn test_recovery_parses_good_statements_around_a_bad_one() {
    // "1; @ ; 2;"  where `@` is a stray Slash with no operands — the middle
    // statement is broken, the outer two are fine.
    let tokens = lex(&[
        Kind::Num(1),
        Kind::Semi,
        Kind::Slash, // bogus leading operator
        Kind::Semi,
        Kind::Num(2),
        Kind::Semi,
    ]);
    let mut p = Parser::new(&tokens);
    let values = parse_program(&mut p);

    // The two well-formed statements still evaluated.
    assert_eq!(values, [1, 2]);
    // The broken one recorded at least one diagnostic.
    assert!(p.has_errors());
}

#[test]
fn test_recovery_makes_progress_and_terminates() {
    // A stream of pure garbage must not loop forever; the program parser drains it.
    let tokens = lex(&[Kind::Slash, Kind::Star, Kind::Plus, Kind::RParen]);
    let mut p = Parser::new(&tokens);
    let values = parse_program(&mut p);
    assert!(values.is_empty());
    assert!(p.at_end());
    assert!(p.has_errors());
}
