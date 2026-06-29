//! The token cursor a recursive-descent grammar threads.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use diag_lang::{Diagnostic, Label, Severity};
use token_lang::{Span, Token, TokenKind};

/// A saved cursor position, taken by [`Parser::checkpoint`] and restored by
/// [`Parser::rewind`].
///
/// A checkpoint captures both the cursor and the number of diagnostics recorded so
/// far, so rewinding to it un-does a speculative parse completely — the cursor
/// moves back and any errors recorded since are dropped. It is `Copy`, so taking
/// one is free and it can be kept in a local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pos: usize,
    errors: usize,
}

/// A cursor over a slice of [`Token`]s, with error recovery, that a hand-written
/// recursive-descent grammar drives.
///
/// `Parser` holds the borrowed token stream, the current position, and the
/// diagnostics recorded so far. A grammar is a set of functions that take
/// `&mut Parser` and return `Option<T>` — `Some(node)` on success, `None` after a
/// recoverable error has been recorded. The cursor skips trivia automatically
/// (anything [`TokenKind::is_trivia`] holds for), so the grammar only ever sees
/// significant tokens, and it stops cleanly at the end of input.
///
/// Kinds are matched with predicates rather than equality — `at(|k| matches!(k,
/// Kind::Plus))` — so a kind that carries data (an interned identifier, a literal)
/// works without a `PartialEq` bound, and matching a *category* never accidentally
/// compares the payload.
///
/// # Examples
///
/// ```
/// use parser_lang::{Parser, Span, Token, TokenKind};
///
/// #[derive(Clone, Copy, PartialEq)]
/// enum Kind { Num, Plus, Eof }
/// impl TokenKind for Kind {
///     fn is_eof(&self) -> bool { matches!(self, Kind::Eof) }
/// }
///
/// // `1 + 2`, terminated.
/// let tokens = [
///     Token::new(Kind::Num, Span::new(0, 1)),
///     Token::new(Kind::Plus, Span::new(2, 3)),
///     Token::new(Kind::Num, Span::new(4, 5)),
///     Token::new(Kind::Eof, Span::empty(5)),
/// ];
///
/// let mut p = Parser::new(&tokens);
/// assert!(p.at(|k| matches!(k, Kind::Num)));
/// p.bump();
/// assert!(p.eat(|k| matches!(k, Kind::Plus)).is_some());
/// assert!(p.at(|k| matches!(k, Kind::Num)));
/// p.bump();
/// assert!(p.at_end());
/// ```
pub struct Parser<'t, K> {
    tokens: &'t [Token<K>],
    /// Index of the current significant token, or `tokens.len()` past the end. The
    /// type's methods keep it parked on a non-trivia token.
    pos: usize,
    /// Where to attribute an error reported at end of input.
    eof_span: Span,
    errors: Vec<Diagnostic>,
}

impl<'t, K: TokenKind> Parser<'t, K> {
    /// Creates a cursor over `tokens`, positioned at the first significant token.
    ///
    /// Leading trivia is skipped immediately. The stream need not end with an
    /// end-of-input token, but if it does the cursor stops on it rather than
    /// running past.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    ///
    /// #[derive(Clone, Copy)]
    /// enum Kind { Word, Space }
    /// impl TokenKind for Kind {
    ///     fn is_trivia(&self) -> bool { matches!(self, Kind::Space) }
    /// }
    ///
    /// // Leading whitespace is skipped on construction.
    /// let tokens = [
    ///     Token::new(Kind::Space, Span::new(0, 1)),
    ///     Token::new(Kind::Word, Span::new(1, 5)),
    /// ];
    /// let p = Parser::new(&tokens);
    /// assert!(p.at(|k| matches!(k, Kind::Word)));
    /// ```
    #[must_use]
    pub fn new(tokens: &'t [Token<K>]) -> Self {
        let eof_span = tokens
            .last()
            .map_or(Span::empty(0), |t| Span::empty(t.span().end().to_u32()));
        let mut parser = Self {
            tokens,
            pos: 0,
            eof_span,
            errors: Vec::new(),
        };
        parser.skip_trivia();
        parser
    }

    /// Advances the cursor past any trivia tokens, parking it on the next
    /// significant token or at the end.
    fn skip_trivia(&mut self) {
        while self.tokens.get(self.pos).is_some_and(Token::is_trivia) {
            self.pos += 1;
        }
    }

    /// Returns the current significant token without consuming it, or `None` at the
    /// end of input.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { A }
    /// # impl TokenKind for K {}
    /// let tokens = [Token::new(K::A, Span::new(0, 1))];
    /// let p = Parser::new(&tokens);
    /// assert_eq!(p.peek().map(|t| t.span()), Some(Span::new(0, 1)));
    /// ```
    #[must_use]
    pub fn peek(&self) -> Option<&'t Token<K>> {
        self.tokens.get(self.pos)
    }

    /// Returns the kind of the current significant token, or `None` at the end.
    #[must_use]
    pub fn peek_kind(&self) -> Option<&'t K> {
        self.peek().map(Token::kind)
    }

    /// Returns the span of the current token, or an empty span at the end of input
    /// (positioned just past the last token), so an error reported at the end still
    /// points somewhere sensible.
    #[must_use]
    pub fn span(&self) -> Span {
        self.peek().map_or(self.eof_span, Token::span)
    }

    /// Returns `true` at the end of input: when there is no current token, or the
    /// current token is the end-of-input marker.
    #[must_use]
    pub fn at_end(&self) -> bool {
        self.peek().is_none_or(Token::is_eof)
    }

    /// Returns `true` if the current token's kind satisfies `pred`. Always `false`
    /// at the end of input.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { Plus, Minus }
    /// # impl TokenKind for K {}
    /// let tokens = [Token::new(K::Plus, Span::new(0, 1))];
    /// let p = Parser::new(&tokens);
    /// assert!(p.at(|k| matches!(k, K::Plus)));
    /// assert!(!p.at(|k| matches!(k, K::Minus)));
    /// ```
    #[must_use]
    pub fn at(&self, pred: impl FnOnce(&K) -> bool) -> bool {
        self.peek_kind().is_some_and(pred)
    }

    /// Consumes and returns the current significant token, advancing to the next
    /// one. Returns `None` (and does not move) at the end of input.
    pub fn bump(&mut self) -> Option<&'t Token<K>> {
        let token = self.tokens.get(self.pos)?;
        self.pos += 1;
        self.skip_trivia();
        Some(token)
    }

    /// Consumes the current token if its kind satisfies `pred`, returning it;
    /// otherwise leaves the cursor untouched and returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { Comma, Num }
    /// # impl TokenKind for K {}
    /// let tokens = [Token::new(K::Num, Span::new(0, 1))];
    /// let mut p = Parser::new(&tokens);
    /// assert!(p.eat(|k| matches!(k, K::Comma)).is_none()); // not a comma
    /// assert!(p.eat(|k| matches!(k, K::Num)).is_some());   // consumed
    /// ```
    pub fn eat(&mut self, pred: impl FnOnce(&K) -> bool) -> Option<&'t Token<K>> {
        if self.at(pred) { self.bump() } else { None }
    }

    /// Consumes the current token if its kind satisfies `pred`; otherwise records an
    /// "expected `{description}`" diagnostic at the current position and returns
    /// `None`.
    ///
    /// This is the workhorse for required tokens: the grammar names what it wanted
    /// (`description`), and on a mismatch the error is recorded for later rendering
    /// while parsing continues — the caller decides whether to recover.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { RParen, Num }
    /// # impl TokenKind for K {}
    /// let tokens = [Token::new(K::Num, Span::new(0, 1))];
    /// let mut p = Parser::new(&tokens);
    /// assert!(p.expect(|k| matches!(k, K::RParen), "`)`").is_none());
    /// assert!(p.has_errors());
    /// ```
    pub fn expect(
        &mut self,
        pred: impl FnOnce(&K) -> bool,
        description: &str,
    ) -> Option<&'t Token<K>> {
        match self.eat(pred) {
            Some(token) => Some(token),
            None => {
                self.error(format!("expected {description}"));
                None
            }
        }
    }

    /// Records an error diagnostic at the current position.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { Bad }
    /// # impl TokenKind for K {}
    /// let tokens = [Token::new(K::Bad, Span::new(0, 3))];
    /// let mut p = Parser::new(&tokens);
    /// p.error("unexpected token");
    /// assert_eq!(p.errors().len(), 1);
    /// ```
    pub fn error(&mut self, message: impl Into<Box<str>>) {
        let span = self.span();
        self.error_at(span, message);
    }

    /// Records an error diagnostic at a specific span — for instance pointing back
    /// at an unclosed opening delimiter rather than at the current token.
    pub fn error_at(&mut self, span: Span, message: impl Into<Box<str>>) {
        self.errors.push(Diagnostic::new(
            Severity::Error,
            message,
            Label::unlabelled(span),
        ));
    }

    /// Skips tokens until the current one satisfies `sync`, or the end of input is
    /// reached, leaving the cursor *on* the synchronizing token.
    ///
    /// This is the recovery primitive: after recording an error, advance to a known
    /// landmark (a statement terminator, a closing brace) and resume parsing there,
    /// so one malformed construct does not derail the rest of the input. It always
    /// makes progress and always stops at the end marker, so it cannot run away.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { Junk, Semi, Eof }
    /// # impl TokenKind for K { fn is_eof(&self) -> bool { matches!(self, K::Eof) } }
    /// let tokens = [
    ///     Token::new(K::Junk, Span::new(0, 1)),
    ///     Token::new(K::Junk, Span::new(1, 2)),
    ///     Token::new(K::Semi, Span::new(2, 3)),
    ///     Token::new(K::Eof, Span::empty(3)),
    /// ];
    /// let mut p = Parser::new(&tokens);
    /// p.recover(|k| matches!(k, K::Semi));
    /// assert!(p.at(|k| matches!(k, K::Semi)));
    /// ```
    pub fn recover(&mut self, sync: impl Fn(&K) -> bool) {
        while let Some(token) = self.tokens.get(self.pos) {
            if token.is_eof() || sync(token.kind()) {
                return;
            }
            self.pos += 1;
            self.skip_trivia();
        }
    }

    /// Parses zero or more items, calling `parse` until it returns `None`, and
    /// collects the results.
    ///
    /// A `parse` that returns `Some` without advancing the cursor would loop
    /// forever; this guards against that by stopping if no progress was made.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { Num, Eof }
    /// # impl TokenKind for K { fn is_eof(&self) -> bool { matches!(self, K::Eof) } }
    /// let tokens = [
    ///     Token::new(K::Num, Span::new(0, 1)),
    ///     Token::new(K::Num, Span::new(1, 2)),
    ///     Token::new(K::Eof, Span::empty(2)),
    /// ];
    /// let mut p = Parser::new(&tokens);
    /// let nums = p.repeated(|p| p.eat(|k| matches!(k, K::Num)).map(|t| t.span()));
    /// assert_eq!(nums.len(), 2);
    /// ```
    pub fn repeated<T>(&mut self, mut parse: impl FnMut(&mut Self) -> Option<T>) -> Vec<T> {
        let mut items = Vec::new();
        loop {
            let before = self.pos;
            match parse(self) {
                Some(item) => {
                    items.push(item);
                    if self.pos == before {
                        break; // no progress: stop rather than spin
                    }
                }
                None => break,
            }
        }
        items
    }

    /// Parses a possibly-empty list of items produced by `parse`, separated by
    /// tokens matching `sep` (such as a comma), and collects the results.
    ///
    /// Parsing stops after a separator that is not followed by another item (a
    /// trailing separator), or when `parse` first returns `None` (an empty list).
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { Num, Comma, Eof }
    /// # impl TokenKind for K { fn is_eof(&self) -> bool { matches!(self, K::Eof) } }
    /// // `1, 2, 3`
    /// let tokens = [
    ///     Token::new(K::Num, Span::new(0, 1)),
    ///     Token::new(K::Comma, Span::new(1, 2)),
    ///     Token::new(K::Num, Span::new(3, 4)),
    ///     Token::new(K::Comma, Span::new(4, 5)),
    ///     Token::new(K::Num, Span::new(6, 7)),
    ///     Token::new(K::Eof, Span::empty(7)),
    /// ];
    /// let mut p = Parser::new(&tokens);
    /// let items = p.separated(
    ///     |k| matches!(k, K::Comma),
    ///     |p| p.eat(|k| matches!(k, K::Num)).map(|t| t.span()),
    /// );
    /// assert_eq!(items.len(), 3);
    /// ```
    pub fn separated<T>(
        &mut self,
        mut sep: impl FnMut(&K) -> bool,
        mut parse: impl FnMut(&mut Self) -> Option<T>,
    ) -> Vec<T> {
        let mut items = Vec::new();
        match parse(self) {
            Some(first) => items.push(first),
            None => return items,
        }
        while self.eat(&mut sep).is_some() {
            let before = self.pos;
            match parse(self) {
                Some(item) => {
                    items.push(item);
                    if self.pos == before {
                        break;
                    }
                }
                None => break,
            }
        }
        items
    }

    /// Takes a snapshot of the cursor and the error count, for speculative parsing.
    ///
    /// Pair it with [`rewind`](Parser::rewind) to try a parse and back out of it
    /// cleanly if it does not work — the cursor returns to where it was and any
    /// diagnostics recorded in the meantime are dropped.
    #[must_use]
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            errors: self.errors.len(),
        }
    }

    /// Restores the cursor and error log to a [`Checkpoint`], undoing everything
    /// done since it was taken.
    ///
    /// # Examples
    ///
    /// ```
    /// use parser_lang::{Parser, Span, Token, TokenKind};
    /// # #[derive(Clone, Copy)] enum K { Num }
    /// # impl TokenKind for K {}
    /// let tokens = [Token::new(K::Num, Span::new(0, 1))];
    /// let mut p = Parser::new(&tokens);
    /// let cp = p.checkpoint();
    /// p.bump();
    /// p.error("speculative");
    /// p.rewind(cp); // cursor and the recorded error are both rolled back
    /// assert!(!p.has_errors());
    /// assert!(p.at(|k| matches!(k, K::Num)));
    /// ```
    pub fn rewind(&mut self, checkpoint: Checkpoint) {
        self.pos = checkpoint.pos;
        self.errors.truncate(checkpoint.errors);
    }

    /// Returns the diagnostics recorded so far, in the order they occurred.
    #[must_use]
    pub fn errors(&self) -> &[Diagnostic] {
        &self.errors
    }

    /// Returns `true` if any diagnostic has been recorded.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Consumes the parser, returning all recorded diagnostics in source order.
    #[must_use]
    pub fn into_errors(self) -> Vec<Diagnostic> {
        self.errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum K {
        Num,
        Plus,
        Space,
        Eof,
    }
    impl TokenKind for K {
        fn is_trivia(&self) -> bool {
            matches!(self, K::Space)
        }
        fn is_eof(&self) -> bool {
            matches!(self, K::Eof)
        }
    }

    fn toks(kinds: &[K]) -> Vec<Token<K>> {
        kinds
            .iter()
            .enumerate()
            .map(|(i, k)| Token::new(*k, Span::new(i as u32, i as u32 + 1)))
            .collect()
    }

    #[test]
    fn test_new_skips_leading_trivia() {
        let t = toks(&[K::Space, K::Space, K::Num]);
        let p = Parser::new(&t);
        assert!(p.at(|k| matches!(k, K::Num)));
    }

    #[test]
    fn test_bump_skips_trailing_trivia() {
        let t = toks(&[K::Num, K::Space, K::Plus]);
        let mut p = Parser::new(&t);
        p.bump();
        assert!(p.at(|k| matches!(k, K::Plus)));
    }

    #[test]
    fn test_navigation_is_total_on_empty_stream() {
        let t: Vec<Token<K>> = Vec::new();
        let mut p = Parser::new(&t);
        assert!(p.peek().is_none());
        assert!(p.at_end());
        assert!(p.bump().is_none());
        assert_eq!(p.span(), Span::empty(0));
    }

    #[test]
    fn test_at_end_true_on_eof_marker() {
        let t = toks(&[K::Num, K::Eof]);
        let mut p = Parser::new(&t);
        assert!(!p.at_end());
        p.bump();
        assert!(p.at_end());
    }

    #[test]
    fn test_expect_records_error_and_returns_none() {
        let t = toks(&[K::Num]);
        let mut p = Parser::new(&t);
        assert!(p.expect(|k| matches!(k, K::Plus), "`+`").is_none());
        assert_eq!(p.errors().len(), 1);
    }

    #[test]
    fn test_recover_stops_at_sync_and_at_eof() {
        let t = toks(&[K::Num, K::Num, K::Plus, K::Eof]);
        let mut p = Parser::new(&t);
        p.recover(|k| matches!(k, K::Plus));
        assert!(p.at(|k| matches!(k, K::Plus)));

        // No sync present -> stop at the eof marker, not past it.
        let t2 = toks(&[K::Num, K::Num, K::Eof]);
        let mut p2 = Parser::new(&t2);
        p2.recover(|k| matches!(k, K::Plus));
        assert!(p2.at_end());
    }

    #[test]
    fn test_checkpoint_rewinds_cursor_and_errors() {
        let t = toks(&[K::Num, K::Plus]);
        let mut p = Parser::new(&t);
        let cp = p.checkpoint();
        p.bump();
        p.error("nope");
        p.rewind(cp);
        assert!(!p.has_errors());
        assert!(p.at(|k| matches!(k, K::Num)));
    }

    #[test]
    fn test_repeated_stops_without_progress() {
        let t = toks(&[K::Num]);
        let mut p = Parser::new(&t);
        // A parse that never consumes returns Some once then would spin; the guard
        // breaks after the first no-progress iteration.
        let collected = p.repeated(|_p| Some(1u8));
        assert_eq!(collected, [1]);
    }
}
