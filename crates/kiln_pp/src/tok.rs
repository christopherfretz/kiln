//! The preprocessor's internal token: a lexer [`Token`] plus flags only
//! the preprocessor needs.

use kiln_lex::{Punct, Symbol, TokFlags, TokKind, Token};

/// Preprocessor-private token flags (clang keeps these in `Token::Flags`
/// or in the token kind; `kiln_lex::TokFlags` cannot be extended).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PpFlags(u8);

impl PpFlags {
    /// This identifier must never be macro-expanded (clang
    /// `Token::DisableExpand`): it named a macro that was disabled
    /// ([cpp.rescan]/2, "painted blue").
    pub(crate) const NO_EXPAND: PpFlags = PpFlags(1);
    /// A `##` token that is not a paste operator: it came from a macro
    /// argument or was formed by pasting (clang turns these into
    /// `tok::unknown`).
    pub(crate) const NOT_PASTE: PpFlags = PpFlags(2);
    /// The result of `#` (clang `Token::StringifiedInMacro`).
    pub(crate) const STRINGIFIED: PpFlags = PpFlags(4);

    pub(crate) const fn empty() -> PpFlags {
        PpFlags(0)
    }

    pub(crate) const fn contains(self, f: PpFlags) -> bool {
        self.0 & f.0 == f.0
    }

    pub(crate) fn insert(&mut self, f: PpFlags) {
        self.0 |= f.0;
    }
}

/// A token with preprocessor flags.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PpTok {
    pub(crate) tok: Token,
    pub(crate) pf: PpFlags,
}

impl PpTok {
    pub(crate) const fn new(tok: Token) -> PpTok {
        PpTok { tok, pf: PpFlags::empty() }
    }

    pub(crate) fn kind(&self) -> TokKind {
        self.tok.kind
    }

    pub(crate) fn ident(&self) -> Option<Symbol> {
        match self.tok.kind {
            TokKind::Identifier(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn is_punct(&self, p: Punct) -> bool {
        self.tok.kind == TokKind::Punct(p)
    }

    /// A `##` that acts as the paste operator.
    pub(crate) fn is_paste_op(&self) -> bool {
        self.is_punct(Punct::HashHash) && !self.pf.contains(PpFlags::NOT_PASTE)
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.tok.kind == TokKind::Eof
    }

    pub(crate) fn is_eod_or_eof(&self) -> bool {
        matches!(self.tok.kind, TokKind::Eod | TokKind::Eof)
    }

    pub(crate) fn has(&self, f: TokFlags) -> bool {
        self.tok.flags.contains(f)
    }

    pub(crate) fn set_flag(&mut self, f: TokFlags, on: bool) {
        if on {
            self.tok.flags.insert(f);
        } else {
            self.tok.flags.remove(f);
        }
    }
}
