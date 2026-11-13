//! Pratt (precedence-climbing) expression parsing.

use alloc::vec::Vec;

use token_lang::{Token, TokenKind};

use crate::Parser;

/// A precedence-climbing expression grammar, driven over a [`Parser`].
///
/// Pratt parsing handles operator precedence and associativity without a separate
/// grammar rule per level. You describe three things and the provided
/// [`expression`](Pratt::expression) driver does the rest:
///
/// - [`prefix`](Pratt::prefix) — parse an operand: a literal, a parenthesized
///   expression, a prefix-unary operator and its operand. This is also where
///   *postfix* forms (a call `()`, an index `[]`) are handled, by looping on the
///   trailing tokens after the atom.
/// - [`infix_binding`](Pratt::infix_binding) — the *binding power* of a kind as an
///   infix operator, as a `(left, right)` pair, or `None` if it is not one. A left
///   power below the right (`(1, 2)`) is left-associative; above (`(4, 3)`) is
///   right-associative.
/// - [`infix`](Pratt::infix) — combine a left operand, the operator token, and the
///   right operand into one result.
///
/// The grammar returns `None` from any method to signal a recoverable error (after
/// recording a diagnostic on the parser); the driver then unwinds with `None`.
///
/// # Examples
///
/// A calculator that evaluates as it parses, so `1 + 2 * 3` is `7` — `*` binds
/// tighter than `+`.
///
/// ```
/// use parser_lang::{Parser, Pratt, Span, Token, TokenKind};
///
/// #[derive(Clone, Copy)]
/// enum K { Num(i64), Plus, Star, Eof }
/// impl TokenKind for K {
///     fn is_eof(&self) -> bool { matches!(self, K::Eof) }
/// }
///
/// struct Calc;
/// impl<'t> Pratt<'t, K> for Calc {
///     type Output = i64;
///     fn prefix(&mut self, p: &mut Parser<'t, K>) -> Option<i64> {
///         match p.bump()?.kind() {
///             K::Num(n) => Some(*n),
///             _ => None,
///         }
///     }
///     fn infix_binding(&self, k: &K) -> Option<(u8, u8)> {
///         match k {
///             K::Plus => Some((1, 2)),
///             K::Star => Some((3, 4)),
///             _ => None,
///         }
///     }
///     fn infix(&mut self, op: &'t Token<K>, left: i64, right: i64) -> Option<i64> {
///         match op.kind() {
///             K::Plus => Some(left + right),
///             K::Star => Some(left * right),
///             _ => None,
///         }
///     }
/// }
///
/// let s = |i| Span::new(i, i + 1);
/// let tokens = [
///     Token::new(K::Num(1), s(0)),
///     Token::new(K::Plus, s(1)),
///     Token::new(K::Num(2), s(2)),
///     Token::new(K::Star, s(3)),
///     Token::new(K::Num(3), s(4)),
///     Token::new(K::Eof, Span::empty(5)),
/// ];
/// let mut p = Parser::new(&tokens);
/// let mut calc = Calc;
/// assert_eq!(calc.parse(&mut p), Some(7));
/// ```
pub trait Pratt<'t, K: TokenKind> {
    /// What the grammar builds — an AST node, a value, a string.
    type Output;

    /// Parses an operand at the cursor: a literal, a parenthesized group, or a
    /// prefix operator applied to a sub-expression (parsed by calling
    /// [`expression`](Pratt::expression) with the operator's right binding power).
    /// Returns `None` after recording a diagnostic on a syntax error.
    fn prefix(&mut self, parser: &mut Parser<'t, K>) -> Option<Self::Output>;

    /// Returns the `(left, right)` binding power of `kind` as an infix operator, or
    /// `None` if the kind does not begin an infix operator. Left below right is
    /// left-associative; left above right is right-associative.
    fn infix_binding(&self, kind: &K) -> Option<(u8, u8)>;

    /// Combines `left`, the already-consumed operator token `op`, and `right` into
    /// one result. Returns `None` after recording a diagnostic on an error.
    fn infix(
        &mut self,
        op: &'t Token<K>,
        left: Self::Output,
        right: Self::Output,
    ) -> Option<Self::Output>;

    /// Parses an expression whose operators all bind at least as tightly as
    /// `min_bp` — the precedence-climbing driver. Provided; do not override.
    ///
    /// Parse a full expression with [`parse`](Pratt::parse), which calls this with
    /// a minimum of `0`. Call it directly with an operator's right binding power
    /// from inside [`prefix`](Pratt::prefix) to parse the operand of a prefix
    /// operator.
    ///
    /// # Depth
    ///
    /// Infix operators never recurse: pending operators wait on a heap stack, so
    /// a chain of any length and associativity (`a = b = c = ...`, `2 ^ 2 ^ ...`)
    /// parses in constant call-stack space.
    ///
    /// Nesting that passes through [`prefix`](Pratt::prefix) (a parenthesized
    /// group, a prefix operator) does recurse, because `prefix` calls back into
    /// this method. Each call counts one level against the parser's
    /// [depth limit](Parser::with_depth_limit). Past it, the call records an
    /// "expression nested too deeply" error at the current token and returns
    /// `None`, so hostile input ends in a diagnostic rather than a stack
    /// overflow.
    fn expression(&mut self, parser: &mut Parser<'t, K>, min_bp: u8) -> Option<Self::Output> {
        if !parser.enter_expression() {
            return None;
        }
        let result = climb(self, parser, min_bp);
        parser.leave_expression();
        result
    }

    /// Parses a complete expression from the cursor (minimum binding power `0`).
    fn parse(&mut self, parser: &mut Parser<'t, K>) -> Option<Self::Output> {
        self.expression(parser, 0)
    }
}

/// The precedence-climbing loop behind [`Pratt::expression`].
///
/// It makes exactly the calls the textbook recursive form makes, in the same
/// order: `prefix` for each operand, `infix_binding` each time an operator is
/// considered, and `infix` as soon as an operator's right operand is complete.
/// Where the recursive form would call itself for a right operand that goes on
/// past its first atom, this loop pushes the operator and its left operand on a
/// heap stack and continues in place; when an operand ends, the top of the
/// stack is popped and combined, and the loop resumes at the popped level's
/// minimum binding power. A right operand that is a single atom (the common
/// case for left-associative chains) is combined at once, without the stack.
fn climb<'t, K, G>(grammar: &mut G, parser: &mut Parser<'t, K>, min_bp: u8) -> Option<G::Output>
where
    K: TokenKind,
    G: Pratt<'t, K> + ?Sized,
{
    // Each entry is a suspended level: its minimum binding power, its left
    // operand, and the operator whose right operand is being parsed.
    let mut pending: Vec<(u8, G::Output, &'t Token<K>)> = Vec::new();
    let mut min = min_bp;
    let mut left = grammar.prefix(parser)?;
    // The right binding power of the operator at the cursor, if it binds at
    // least as tightly as `min` and so continues the current level.
    let mut next = continues(grammar, parser, min);
    loop {
        if let Some(right_bp) = next {
            // Consume the operator, then parse its right operand at the
            // operator's right binding power: a higher right power groups more
            // to the right.
            let op = parser.bump()?;
            let right = grammar.prefix(parser)?;
            let inner = continues(grammar, parser, right_bp);
            if inner.is_some() {
                // The right operand goes on: suspend this level.
                pending.push((min, left, op));
                min = right_bp;
                left = right;
                next = inner;
            } else {
                left = grammar.infix(op, left, right)?;
                next = continues(grammar, parser, min);
            }
            continue;
        }
        // The current operand is complete. Hand it to the operator waiting for
        // it, or return it if none is.
        let Some((outer_min, outer_left, op)) = pending.pop() else {
            return Some(left);
        };
        left = grammar.infix(op, outer_left, left)?;
        min = outer_min;
        next = continues(grammar, parser, min);
    }
}

/// Returns the right binding power of the infix operator at the cursor if its
/// left binding power is at least `min_bp`, or `None` if the cursor is not on
/// such an operator.
fn continues<'t, K, G>(grammar: &G, parser: &Parser<'t, K>, min_bp: u8) -> Option<u8>
where
    K: TokenKind,
    G: Pratt<'t, K> + ?Sized,
{
    let kind = parser.peek_kind()?;
    let (left_bp, right_bp) = grammar.infix_binding(kind)?;
    (left_bp >= min_bp).then_some(right_bp)
}

#[cfg(test)]
mod tests {
    use token_lang::Span;

    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum K {
        Num(i64),
        Plus,
        Minus,
        Star,
        Caret,
        Eof,
    }
    impl TokenKind for K {
        fn is_eof(&self) -> bool {
            matches!(self, K::Eof)
        }
    }

    /// Builds an `S`-expression string so associativity and precedence are visible
    /// in the output: `(+ 1 (* 2 3))`.
    struct Sexpr;
    impl<'t> Pratt<'t, K> for Sexpr {
        type Output = alloc::string::String;
        fn prefix(&mut self, p: &mut Parser<'t, K>) -> Option<Self::Output> {
            use alloc::string::ToString;
            match p.bump()?.kind() {
                K::Num(n) => Some(n.to_string()),
                K::Minus => {
                    // Prefix negation binds tighter than any infix operator here.
                    let operand = self.expression(p, 100)?;
                    Some(alloc::format!("(neg {operand})"))
                }
                _ => None,
            }
        }
        fn infix_binding(&self, k: &K) -> Option<(u8, u8)> {
            match k {
                K::Plus | K::Minus => Some((1, 2)),
                K::Star => Some((3, 4)),
                K::Caret => Some((6, 5)), // right-associative
                _ => None,
            }
        }
        fn infix(
            &mut self,
            op: &'t Token<K>,
            left: Self::Output,
            right: Self::Output,
        ) -> Option<Self::Output> {
            let sym = match op.kind() {
                K::Plus => "+",
                K::Minus => "-",
                K::Star => "*",
                K::Caret => "^",
                _ => return None,
            };
            Some(alloc::format!("({sym} {left} {right})"))
        }
    }

    fn lex(kinds: &[K]) -> alloc::vec::Vec<Token<K>> {
        kinds
            .iter()
            .enumerate()
            .map(|(i, k)| Token::new(*k, Span::new(i as u32, i as u32 + 1)))
            .collect()
    }

    fn parse(kinds: &[K]) -> Option<alloc::string::String> {
        let tokens = lex(kinds);
        let mut p = Parser::new(&tokens);
        Sexpr.parse(&mut p)
    }

    #[test]
    fn test_precedence_multiplication_binds_tighter() {
        // 1 + 2 * 3  ->  (+ 1 (* 2 3))
        let out = parse(&[K::Num(1), K::Plus, K::Num(2), K::Star, K::Num(3), K::Eof]);
        assert_eq!(out.as_deref(), Some("(+ 1 (* 2 3))"));
    }

    #[test]
    fn test_left_associative_addition() {
        // 1 - 2 - 3  ->  (- (- 1 2) 3)
        let out = parse(&[K::Num(1), K::Minus, K::Num(2), K::Minus, K::Num(3), K::Eof]);
        assert_eq!(out.as_deref(), Some("(- (- 1 2) 3)"));
    }

    #[test]
    fn test_right_associative_power() {
        // 2 ^ 3 ^ 2  ->  (^ 2 (^ 3 2))
        let out = parse(&[K::Num(2), K::Caret, K::Num(3), K::Caret, K::Num(2), K::Eof]);
        assert_eq!(out.as_deref(), Some("(^ 2 (^ 3 2))"));
    }

    #[test]
    fn test_prefix_operator() {
        // -2 * 3  ->  (* (neg 2) 3)
        let out = parse(&[K::Minus, K::Num(2), K::Star, K::Num(3), K::Eof]);
        assert_eq!(out.as_deref(), Some("(* (neg 2) 3)"));
    }
}
