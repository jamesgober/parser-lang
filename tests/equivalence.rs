//! The Pratt driver keeps pending operators on a heap stack instead of
//! recursing. These properties hold it to the textbook recursive driver: for
//! arbitrary binding-power tables (left, right and odd associativity, kinds
//! that are not infix operators at all) and arbitrary token streams, both make
//! the same grammar calls in the same order and end with the same result,
//! cursor position and errors.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;

use parser_lang::{Parser, Pratt, Span, Token, TokenKind};
use proptest::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Num(u8),
    Op(u8),
    Neg,
    Open,
    Close,
    Eof,
}

impl TokenKind for Kind {
    fn is_eof(&self) -> bool {
        matches!(self, Kind::Eof)
    }
}

/// A grammar that logs every call a driver makes into it.
struct Tracer {
    /// Binding powers per operator; `None` means "not an infix operator".
    table: Vec<Option<(u8, u8)>>,
    /// Drive nested expressions with the recursive reference instead.
    reference: bool,
    /// Make `infix` fail for this operator, to exercise early returns.
    failing_op: Option<u8>,
    log: RefCell<Vec<String>>,
}

impl Tracer {
    fn new(table: &[Option<(u8, u8)>], reference: bool, failing_op: Option<u8>) -> Self {
        Self {
            table: table.to_vec(),
            reference,
            failing_op,
            log: RefCell::new(Vec::new()),
        }
    }

    fn note(&self, entry: String) {
        self.log.borrow_mut().push(entry);
    }

    fn nested(&mut self, p: &mut Parser<'_, Kind>, min_bp: u8) -> Option<String> {
        if self.reference {
            reference(self, p, min_bp)
        } else {
            self.expression(p, min_bp)
        }
    }
}

impl<'t> Pratt<'t, Kind> for Tracer {
    type Output = String;

    fn prefix(&mut self, p: &mut Parser<'t, Kind>) -> Option<String> {
        self.note(format!("prefix@{}", p.span().start().to_u32()));
        match p.bump()?.kind() {
            Kind::Num(n) => Some(n.to_string()),
            Kind::Neg => {
                let operand = self.nested(p, 50)?;
                Some(format!("(neg {operand})"))
            }
            Kind::Open => {
                let inner = self.nested(p, 0)?;
                p.expect(|k| matches!(k, Kind::Close), "`)`")?;
                Some(format!("[{inner}]"))
            }
            _ => {
                p.error("expected an operand");
                None
            }
        }
    }

    fn infix_binding(&self, kind: &Kind) -> Option<(u8, u8)> {
        self.note(format!("binding {kind:?}"));
        match kind {
            Kind::Op(i) => self.table.get(usize::from(*i)).copied().flatten(),
            _ => None,
        }
    }

    fn infix(&mut self, op: &'t Token<Kind>, left: String, right: String) -> Option<String> {
        let Kind::Op(i) = *op.kind() else {
            return None;
        };
        self.note(format!("infix{i}@{}", op.span().start().to_u32()));
        if self.failing_op == Some(i) {
            return None;
        }
        Some(format!("(o{i} {left} {right})"))
    }
}

/// The recursive precedence-climbing driver parser-lang used before its driver
/// became iterative, kept here as the specification.
fn reference(g: &mut Tracer, parser: &mut Parser<'_, Kind>, min_bp: u8) -> Option<String> {
    let mut left = g.prefix(parser)?;
    while let Some(kind) = parser.peek_kind() {
        let Some((left_bp, right_bp)) = g.infix_binding(kind) else {
            break;
        };
        if left_bp < min_bp {
            break;
        }
        let op = parser.bump()?;
        let right = reference(g, parser, right_bp)?;
        left = g.infix(op, left, right)?;
    }
    Some(left)
}

type Outcome = (Option<String>, Vec<String>, u32, Vec<String>);

fn tokens(kinds: &[Kind]) -> Vec<Token<Kind>> {
    let mut out: Vec<Token<Kind>> = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| Token::new(*k, Span::new(i as u32, i as u32 + 1)))
        .collect();
    out.push(Token::new(Kind::Eof, Span::empty(kinds.len() as u32)));
    out
}

fn run(
    table: &[Option<(u8, u8)>],
    kinds: &[Kind],
    failing: Option<u8>,
    use_reference: bool,
) -> Outcome {
    let toks = tokens(kinds);
    let mut p = Parser::new(&toks).with_depth_limit(usize::MAX);
    let mut g = Tracer::new(table, use_reference, failing);
    let out = if use_reference {
        reference(&mut g, &mut p, 0)
    } else {
        g.parse(&mut p)
    };
    let errors = p.errors().iter().map(|d| d.message().to_string()).collect();
    (out, g.log.into_inner(), p.span().start().to_u32(), errors)
}

fn arb_kind(ops: u8) -> impl Strategy<Value = Kind> {
    prop_oneof![
        6 => (0u8..10).prop_map(Kind::Num),
        6 => (0..ops).prop_map(Kind::Op),
        1 => Just(Kind::Neg),
        1 => Just(Kind::Open),
        1 => Just(Kind::Close),
    ]
}

/// A binding table, a token stream, and an operator whose `infix` fails.
type Case = (Vec<Option<(u8, u8)>>, Vec<Kind>, Option<u8>);

fn arb_case() -> impl Strategy<Value = Case> {
    let table = prop::collection::vec(prop::option::weighted(0.85, (0u8..8, 0u8..8)), 1..6);
    table.prop_flat_map(|table| {
        let ops = table.len() as u8;
        (
            Just(table),
            prop::collection::vec(arb_kind(ops), 0..40),
            prop::option::weighted(0.2, 0..ops),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn driver_matches_recursive_reference((table, kinds, failing) in arb_case()) {
        prop_assert_eq!(run(&table, &kinds, failing, false), run(&table, &kinds, failing, true));
    }
}

#[test]
fn fixed_associativity_cases() {
    // op 0 left (1, 2), op 1 right (6, 5), op 2 right (2, 1), op 3 left (3, 4).
    let table = [Some((1, 2)), Some((6, 5)), Some((2, 1)), Some((3, 4))];
    let n = Kind::Num;
    let o = Kind::Op;
    let cases: &[(&[Kind], &str)] = &[
        (&[n(1), o(0), n(2), o(0), n(3)], "(o0 (o0 1 2) 3)"),
        (&[n(1), o(1), n(2), o(1), n(3)], "(o1 1 (o1 2 3))"),
        (&[n(1), o(2), n(2), o(2), n(3)], "(o2 1 (o2 2 3))"),
        (&[n(1), o(0), n(2), o(3), n(3)], "(o0 1 (o3 2 3))"),
        (&[n(1), o(3), n(2), o(0), n(3)], "(o0 (o3 1 2) 3)"),
        (
            &[n(1), o(2), n(2), o(0), n(3), o(1), n(4)],
            "(o2 1 (o0 2 (o1 3 4)))",
        ),
        (&[Kind::Neg, n(1), o(1), n(2)], "(o1 (neg 1) 2)"),
        (
            &[Kind::Open, n(1), o(0), n(2), Kind::Close, o(3), n(3)],
            "(o3 [(o0 1 2)] 3)",
        ),
    ];
    for (kinds, want) in cases {
        let driver = run(&table, kinds, None, false);
        assert_eq!(driver.0.as_deref(), Some(*want), "{kinds:?}");
        assert_eq!(driver, run(&table, kinds, None, true));
    }
}
