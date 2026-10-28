//! Preprocessing tokens: [`Token`], [`TokKind`], [`TokFlags`], [`Punct`].

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

use kiln_source::Span;

use crate::interner::Symbol;

/// One preprocessing token ([lex.pptoken]).
///
/// Tokens are small (`Copy`, 20 bytes) and carry no text: the spelling is
/// recovered from the source through [`crate::spelling`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Token {
    /// What kind of pp-token this is.
    pub kind: TokKind,
    /// The bytes of the token in the source, including any line splices
    /// inside it (and leading splices, as in clang: a token that starts
    /// with `\`-newline begins at the backslash). Empty for `Eod` / `Eof`.
    pub span: Span,
    /// Whitespace and spelling flags.
    pub flags: TokFlags,
}

impl Token {
    /// True if this is [`TokKind::Eof`].
    pub fn is_eof(&self) -> bool {
        self.kind == TokKind::Eof
    }

    /// True if `flag` is set.
    pub fn has(&self, flag: TokFlags) -> bool {
        self.flags.contains(flag)
    }
}

/// The encoding prefix of a character or string literal ([lex.ccon],
/// [lex.string]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Encoding {
    /// No prefix.
    Ordinary,
    /// `L`.
    Wide,
    /// `u8`.
    Utf8,
    /// `u`.
    Utf16,
    /// `U`.
    Utf32,
}

impl Encoding {
    /// The prefix as written (`""`, `"L"`, `"u8"`, `"u"`, `"U"`).
    pub const fn prefix(self) -> &'static str {
        match self {
            Encoding::Ordinary => "",
            Encoding::Wide => "L",
            Encoding::Utf8 => "u8",
            Encoding::Utf16 => "u",
            Encoding::Utf32 => "U",
        }
    }

    /// clang's token-kind prefix (`""`, `"wide_"`, `"utf8_"`, ...).
    const fn clang_prefix(self) -> &'static str {
        match self {
            Encoding::Ordinary => "",
            Encoding::Wide => "wide_",
            Encoding::Utf8 => "utf8_",
            Encoding::Utf16 => "utf16_",
            Encoding::Utf32 => "utf32_",
        }
    }
}

/// The kind of a preprocessing token.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TokKind {
    /// An identifier ([lex.name]). The symbol is the identifier with
    /// universal-character-names resolved (`é` and `é` are the same
    /// symbol); keywords and alternative tokens (`and`, `bitor`, ...) are
    /// identifiers at this level.
    Identifier(Symbol),
    /// A preprocessing number ([lex.ppnumber]).
    PpNumber,
    /// A character literal, including any ud-suffix ([lex.ccon]).
    CharLit(Encoding),
    /// A (non-raw) string literal, including any ud-suffix ([lex.string]).
    StringLit(Encoding),
    /// A raw string literal `R"d(...)d"`, including any ud-suffix.
    RawStringLit(Encoding),
    /// A header name ([lex.header]); produced only in [`crate::LexMode::HeaderName`].
    HeaderName,
    /// A preprocessing-op-or-punc ([lex.operators]). Digraphs are mapped to
    /// their primary spelling and carry [`TokFlags::DIGRAPH`].
    Punct(Punct),
    /// A single non-whitespace character that cannot begin any other token
    /// ([lex.pptoken]/2): `@`, `` ` ``, a lone `\`, control characters, `$`
    /// when it is not an identifier character, and stray non-identifier
    /// Unicode characters (diagnosed).
    Other(char),
    /// An error-recovery token (clang's `tok::unknown`): an unterminated
    /// character or string literal, a raw string literal with a bad
    /// delimiter or no terminator, an empty character literal, or a
    /// universal-character-name that cannot start an identifier. The
    /// lexer has recorded a diagnostic for every case except the last.
    Unknown,
    /// A comment; produced only with [`crate::LexOptions::keep_comments`].
    Comment,
    /// End of a preprocessing directive: the newline (or end of input)
    /// that ends a line lexed with `in_directive` set.
    Eod,
    /// End of input. Returned repeatedly once reached.
    Eof,
}

impl TokKind {
    /// A stable name used by [`crate::dump`]: clang's token-kind name where
    /// one exists (`identifier`, `numeric_constant`, `l_paren`,
    /// `utf8_string_literal`, ...), else a kiln name (`raw_string_literal`,
    /// `other`, `comment`).
    pub fn name(self) -> String {
        match self {
            TokKind::Identifier(_) => "identifier".to_owned(),
            TokKind::PpNumber => "numeric_constant".to_owned(),
            TokKind::CharLit(e) => format!("{}char_constant", e.clang_prefix()),
            TokKind::StringLit(e) => format!("{}string_literal", e.clang_prefix()),
            TokKind::RawStringLit(e) => format!("{}raw_string_literal", e.clang_prefix()),
            TokKind::HeaderName => "header_name".to_owned(),
            TokKind::Punct(p) => p.name().to_owned(),
            TokKind::Other(_) => "other".to_owned(),
            TokKind::Unknown => "unknown".to_owned(),
            TokKind::Comment => "comment".to_owned(),
            TokKind::Eod => "eod".to_owned(),
            TokKind::Eof => "eof".to_owned(),
        }
    }
}

/// Token flags (a `u8` bit set).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TokFlags(u8);

impl TokFlags {
    /// Whitespace or a comment precedes the token on the same logical line.
    pub const LEADING_SPACE: TokFlags = TokFlags(1 << 0);
    /// The token is the first on its line: a newline (outside comments)
    /// or the start of input precedes it.
    pub const START_OF_LINE: TokFlags = TokFlags(1 << 1);
    /// The token's source bytes contain a line splice (or, with trigraphs
    /// enabled, a trigraph), so its spelling differs from its source slice.
    pub const NEEDS_CLEANING: TokFlags = TokFlags(1 << 2);
    /// The punctuator was spelled as a digraph (`<:` `:>` `<%` `%>` `%:` `%:%:`).
    pub const DIGRAPH: TokFlags = TokFlags(1 << 3);
    /// The identifier or pp-number contains a universal-character-name.
    pub const HAS_UCN: TokFlags = TokFlags(1 << 4);
    /// The character or string literal has a ud-suffix ([lex.ext]).
    pub const UD_SUFFIX: TokFlags = TokFlags(1 << 5);

    const NAMES: [(TokFlags, &'static str); 6] = [
        (TokFlags::START_OF_LINE, "StartOfLine"),
        (TokFlags::LEADING_SPACE, "LeadingSpace"),
        (TokFlags::NEEDS_CLEANING, "UnClean"),
        (TokFlags::DIGRAPH, "Digraph"),
        (TokFlags::HAS_UCN, "UCN"),
        (TokFlags::UD_SUFFIX, "UDSuffix"),
    ];

    /// No flags.
    pub const fn empty() -> TokFlags {
        TokFlags(0)
    }

    /// The raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True if every flag in `other` is set in `self`.
    pub const fn contains(self, other: TokFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// True if no flag is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Sets the flags in `other`.
    pub fn insert(&mut self, other: TokFlags) {
        self.0 |= other.0;
    }

    /// Clears the flags in `other`.
    pub fn remove(&mut self, other: TokFlags) {
        self.0 &= !other.0;
    }

    /// Calls `f` with the dump name of each set flag, in a fixed order.
    pub(crate) fn for_each_name(self, mut f: impl FnMut(&'static str)) {
        for (flag, name) in Self::NAMES {
            if self.contains(flag) {
                f(name);
            }
        }
    }
}

impl BitOr for TokFlags {
    type Output = TokFlags;
    fn bitor(self, rhs: TokFlags) -> TokFlags {
        TokFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for TokFlags {
    fn bitor_assign(&mut self, rhs: TokFlags) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for TokFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut set = f.debug_set();
        self.for_each_name(|n| {
            set.entry(&format_args!("{n}"));
        });
        set.finish()
    }
}

macro_rules! puncts {
    ($($(#[$doc:meta])* $variant:ident = $spelling:literal, $clang:literal;)*) => {
        /// A preprocessing-op-or-punc ([lex.operators]), by primary spelling.
        ///
        /// The alternative tokens that are identifiers (`and`, `or`,
        /// `bitand`, ...) are not here: they lex as identifiers and are
        /// classified by later phases.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum Punct {
            $($(#[$doc])* #[doc = concat!("`", $spelling, "`")] $variant,)*
        }

        impl Punct {
            /// Every punctuator, in declaration order.
            pub const ALL: &'static [Punct] = &[$(Punct::$variant,)*];

            /// The primary spelling (digraphs map to the token they stand for).
            pub const fn as_str(self) -> &'static str {
                match self { $(Punct::$variant => $spelling,)* }
            }

            /// clang's token-kind name (`l_square`, `coloncolon`, ...).
            pub const fn name(self) -> &'static str {
                match self { $(Punct::$variant => $clang,)* }
            }
        }
    };
}

puncts! {
    LBrace = "{", "l_brace";
    RBrace = "}", "r_brace";
    LSquare = "[", "l_square";
    RSquare = "]", "r_square";
    LParen = "(", "l_paren";
    RParen = ")", "r_paren";
    Semi = ";", "semi";
    Colon = ":", "colon";
    Ellipsis = "...", "ellipsis";
    Question = "?", "question";
    ColonColon = "::", "coloncolon";
    Period = ".", "period";
    PeriodStar = ".*", "periodstar";
    Arrow = "->", "arrow";
    ArrowStar = "->*", "arrowstar";
    Tilde = "~", "tilde";
    Exclaim = "!", "exclaim";
    Plus = "+", "plus";
    Minus = "-", "minus";
    Star = "*", "star";
    Slash = "/", "slash";
    Percent = "%", "percent";
    Caret = "^", "caret";
    Amp = "&", "amp";
    Pipe = "|", "pipe";
    Equal = "=", "equal";
    PlusEqual = "+=", "plusequal";
    MinusEqual = "-=", "minusequal";
    StarEqual = "*=", "starequal";
    SlashEqual = "/=", "slashequal";
    PercentEqual = "%=", "percentequal";
    CaretEqual = "^=", "caretequal";
    AmpEqual = "&=", "ampequal";
    PipeEqual = "|=", "pipeequal";
    EqualEqual = "==", "equalequal";
    ExclaimEqual = "!=", "exclaimequal";
    Less = "<", "less";
    Greater = ">", "greater";
    LessEqual = "<=", "lessequal";
    GreaterEqual = ">=", "greaterequal";
    /// C++20 and later; earlier it lexes as `<=` `>`.
    Spaceship = "<=>", "spaceship";
    AmpAmp = "&&", "ampamp";
    PipePipe = "||", "pipepipe";
    LessLess = "<<", "lessless";
    GreaterGreater = ">>", "greatergreater";
    LessLessEqual = "<<=", "lesslessequal";
    GreaterGreaterEqual = ">>=", "greatergreaterequal";
    PlusPlus = "++", "plusplus";
    MinusMinus = "--", "minusminus";
    Comma = ",", "comma";
    Hash = "#", "hash";
    HashHash = "##", "hashhash";
    /// C++26 reflection operator (P2996); earlier it lexes as `^` `^`.
    CaretCaret = "^^", "caretcaret";
    /// C++26 splice-specifier opener (P2996); earlier it lexes as `[` `:`.
    SpliceOpen = "[:", "l_splice";
    /// C++26 splice-specifier closer (P2996); earlier it lexes as `:` `]`.
    SpliceClose = ":]", "r_splice";
}

const _: () = assert!(size_of::<Token>() <= 20);
const _: () = assert!(size_of::<TokKind>() == 8);
