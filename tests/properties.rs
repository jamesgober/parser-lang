//! Property tests. The Pratt engine's precedence and associativity are validated
//! against an independent shunting-yard oracle, and the cursor is shown to be
//! total — it never panics and always terminates — on arbitrary token streams.

#![allow(clippy::unwrap_used)]

use parser_lang::{Parser, Pratt, Span, Token, TokenKind};
use proptest::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Num(i64),
    Op(char),
    Junk,
    Eof,
}

impl TokenKind for Kind {
    fn is_eof(&self) -> bool {
        matches!(self, Kind::Eof)
    }
}

/// `(left, right)` binding power and whether the operator is left-associative.
fn binding(op: char) -> (u8, u8) {
    match op {
        '+' | '-' => (1, 2),
        '*' | '/' => (3, 4),
        '^' => (6, 5), // right-associative
        _ => (0, 0),
    }
}

/// The Pratt grammar under test: builds an S-expression so precedence and
/// associativity are visible in the structure.
struct Sexpr;

impl<'t> Pratt<'t, Kind> for Sexpr {
    type Output = String;

    fn prefix(&mut self, p: &mut Parser<'t, Kind>) -> Option<String> {
        match p.bump()?.kind() {
            Kind::Num(n) => Some(n.to_string()),
            _ => None,
        }
    }
    fn infix_binding(&self, kind: &Kind) -> Option<(u8, u8)> {
        match kind {
            Kind::Op(op) => Some(binding(*op)),
            _ => None,
        }
    }
    fn infix(&mut self, op: &'t Token<Kind>, left: String, right: String) -> Option<String> {
        match op.kind() {
            Kind::Op(op) => Some(format!("({op} {left} {right})")),
            _ => None,
        }
    }
}

/// The oracle: a shunting-yard reduction to the same S-expression form, an
/// algorithm independent of precedence climbing. `items` alternates operand,
/// operator, operand, ... with an odd length.
fn shunting_yard(items: &[(i64, Option<char>)]) -> String {
    let mut operands: Vec<String> = Vec::new();
    let mut ops: Vec<char> = Vec::new();

    let apply = |operands: &mut Vec<String>, op: char| {
        let right = operands.pop().unwrap();
        let left = operands.pop().unwrap();
        operands.push(format!("({op} {left} {right})"));
    };

    for (num, op) in items {
        operands.push(num.to_string());
        if let Some(op) = op {
            let (op_left, op_right) = binding(*op);
            while let Some(&top) = ops.last() {
                let (top_left, _) = binding(top);
                // Pop the stacked operator if it binds tighter, or equally tight
                // and the incoming operator is left-associative (left < right).
                let incoming_left_assoc = op_left < op_right;
                if top_left > op_left || (top_left == op_left && incoming_left_assoc) {
                    apply(&mut operands, top);
                    ops.pop();
                } else {
                    break;
                }
            }
            ops.push(*op);
        }
    }
    while let Some(op) = ops.pop() {
        apply(&mut operands, op);
    }
    operands.pop().unwrap()
}

/// Lays out `items` as a token stream: `num op num op ... num`, then `Eof`.
fn lex(items: &[(i64, Option<char>)]) -> Vec<Token<Kind>> {
    let mut tokens = Vec::new();
    let mut at = 0u32;
    for (num, op) in items {
        tokens.push(Token::new(Kind::Num(*num), Span::new(at, at + 1)));
        at += 1;
        if let Some(op) = op {
            tokens.push(Token::new(Kind::Op(*op), Span::new(at, at + 1)));
            at += 1;
        }
    }
    tokens.push(Token::new(Kind::Eof, Span::empty(at)));
    tokens
}

proptest! {
    /// The Pratt parser groups operators exactly as the shunting-yard oracle does,
    /// for any sequence of operands and operators.
    #[test]
    fn pratt_matches_shunting_yard_oracle(
        nums in prop::collection::vec(-9i64..=9, 1..12),
        ops in prop::collection::vec(prop::sample::select(vec!['+', '-', '*', '/', '^']), 0..12),
    ) {
        // Build a well-formed alternating sequence `num op num op ... num`:
        // `n_ops` operators joining `n_ops + 1` operands, so only the final operand
        // has no trailing operator.
        let n_ops = nums.len().saturating_sub(1).min(ops.len());
        let mut items: Vec<(i64, Option<char>)> = Vec::new();
        for i in 0..=n_ops {
            let op = if i < n_ops { Some(ops[i]) } else { None };
            items.push((nums[i], op));
        }

        let expected = shunting_yard(&items);

        let tokens = lex(&items);
        let mut p = Parser::new(&tokens);
        let got = Sexpr.parse(&mut p);

        prop_assert_eq!(got.as_deref(), Some(expected.as_str()));
        prop_assert!(!p.has_errors());
    }

    /// The cursor is total: navigating an arbitrary token stream with a mix of
    /// peek / bump / eat / recover never panics and always terminates.
    #[test]
    fn cursor_is_total_on_arbitrary_streams(
        kinds in prop::collection::vec(
            prop_oneof![
                (-9i64..=9).prop_map(Kind::Num),
                Just(Kind::Op('+')),
                Just(Kind::Junk),
                Just(Kind::Eof),
            ],
            0..40,
        ),
    ) {
        let tokens: Vec<Token<Kind>> = kinds
            .iter()
            .enumerate()
            .map(|(i, k)| Token::new(*k, Span::new(i as u32, i as u32 + 1)))
            .collect();
        let mut p = Parser::new(&tokens);

        // Drain the stream with a bounded loop; `bump` returning None ends it.
        let mut steps = 0;
        while !p.at_end() {
            let _ = p.eat(|k| matches!(k, Kind::Num(_)));
            p.recover(|k| matches!(k, Kind::Op('+')));
            if p.bump().is_none() {
                break;
            }
            steps += 1;
            prop_assert!(steps <= tokens.len() + 1); // guaranteed progress
        }
        prop_assert!(p.at_end());
    }
}
