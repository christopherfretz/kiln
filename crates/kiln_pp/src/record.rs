//! The preprocessing record (spec/pp.md "Preprocessing record"): what
//! macro expansion did, kept on request ([`crate::PpOptions::pp_record`])
//! so that a tooling client can ask where a token location came from.
//!
//! Nothing is stored per token. The record keeps the definitions (with
//! their history), one row per expansion with its argument spans, and a
//! side index of the source-manager entries made in macro expansion;
//! [`PpRecord::origin`] reads a location's entry and finds it there.
//!
//! It also keeps one row per conditional directive (live and dead, with
//! its outcome and its links to the rest of its conditional), one per
//! inclusion directive (with its outcome and the instance it entered),
//! and the `#pragma once` that took effect; [`PpRecord::inclusion_of`]
//! gives a location's include context.

use std::fmt::Write as _;
use std::rc::Rc;

use kiln_lex::{Interner, Symbol};
use kiln_source::fx::FxHashMap;
use kiln_source::{ExpansionKind, FileId, Loc, SourceManager, Span};

use kiln_snap::Snap;

use crate::macros::MacroDef;

mod carry;
mod stage3;
pub use carry::RecordCarry;
pub use stage3::{Made, Query, Read, ReadKind, StackEvent, StackKind};

/// A definition's index in [`PpRecord::defs`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Snap)]
pub struct DefIx(u32);

/// An expansion's index in [`PpRecord::exps`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Snap)]
pub struct ExpIx(u32);

impl DefIx {
    /// The index.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl ExpIx {
    /// The index.
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// The expansion at position `i` of [`PpRecord::exps`].
    ///
    /// # Panics
    /// If `i` does not fit in a `u32`.
    pub fn from_index(i: usize) -> ExpIx {
        ExpIx(u32::try_from(i).expect("an expansion index"))
    }
}

/// No expansion (a `u32` slot).
const NONE: u32 = u32::MAX;
/// Expansion indices fit below the flag bits of [`Expansion::parent`].
const IX_MASK: u32 = 0x0FFF_FFFF;

/// Where a definition came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Snap)]
pub enum DefSource {
    /// A `#define` in a file.
    File,
    /// A builtin macro (`__LINE__`, `__has_include`, `_Pragma`, ...).
    Builtin,
    /// The predefines text of `<built-in>`.
    Predefined,
    /// The `<command line>` part of the predefines buffer (`-D`, `-U`) or
    /// [`crate::PpOptions::defines`] / `undefines`.
    CommandLine,
    /// Not seen being defined: restored from a PCH snapshot or a
    /// preprocessor-state record, or imported; registered when first met.
    Restored,
}

/// How a definition stopped being in effect (the latest event).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Snap)]
pub enum DefEnd {
    /// Still in effect (or made current again by `pop_macro`).
    Live,
    /// Ended by the `#undef` (or `-U`) whose `#` is at the location.
    Undef(Loc),
    /// Replaced by a redefinition.
    Replaced(DefIx),
    /// Replaced by a `pop_macro`: the location is its event's `at` (the
    /// `#`, or the `_Pragma` token), see [`PpRecord::stack_events`].
    Popped(Loc),
}

/// One definition.
#[derive(Debug, Snap)]
pub struct Definition {
    /// The definition.
    pub def: Rc<MacroDef>,
    /// Where it came from.
    pub source: DefSource,
    /// The `#` of the `#define` (invalid for builtin and restored ones).
    pub hash: Loc,
    /// The end of the directive: the newline of its last line (invalid
    /// like `hash`).
    pub end: Loc,
    /// How it ended.
    pub ended: DefEnd,
    /// Start of its parameter locations in `params`.
    params: u32,
}

/// Flags of one expansion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Snap)]
pub struct ExpFlags(u8);

impl ExpFlags {
    /// Expanded inside a directive (`#if`, `#include`, ...).
    pub const IN_DIRECTIVE: ExpFlags = ExpFlags(1);
    /// The variadic argument was omitted (`F(1)` for `F(a, ...)`).
    pub const VA_ELIDED: ExpFlags = ExpFlags(2);
    /// Error recovery regrouped the arguments (too many arguments, a
    /// braced list split by commas): a client should not edit inside it.
    pub const REGROUPED: ExpFlags = ExpFlags(4);

    /// No flag.
    pub const fn empty() -> ExpFlags {
        ExpFlags(0)
    }

    pub(crate) const fn from_bits(b: u8) -> ExpFlags {
        ExpFlags(b)
    }

    /// All of `f` are set.
    pub const fn contains(self, f: ExpFlags) -> bool {
        self.0 & f.0 == f.0
    }

    /// The flags as bits.
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// One expansion (24 bytes).
#[derive(Clone, Copy, Debug, Snap)]
pub struct Expansion {
    /// The definition expanded.
    pub def: DefIx,
    /// The invocation's name token.
    pub name: Loc,
    /// The `)` of a function-like invocation, else the name (may be in
    /// another entry than `name`).
    pub end: Loc,
    /// Base of the result's entry (replacement list, the single token of a
    /// trivial expansion, a builtin's result); invalid for an empty one.
    pub entry: Loc,
    /// `ExpIx` of the parent (or `IX_MASK`), flags in the top 4 bits.
    parent: u32,
    /// Start of its arguments in `args`.
    args: u32,
}

/// One argument row: its token locations start at `tok_start` in
/// `arg_toks`; `hi` is the end of its last token, or the `,`/`)` that
/// ended it when it is empty.
#[derive(Clone, Copy, Debug, Snap)]
struct ArgRow {
    tok_start: u32,
    hi: Loc,
}

/// One argument of an expansion, as collected (before pre-expansion).
#[derive(Clone, Copy, Debug)]
pub struct Arg<'a> {
    /// From the first token to the end of the last; empty at the `,` or
    /// `)` that ended an empty argument.
    pub span: Span,
    /// The locations of its tokens.
    pub toks: &'a [Loc],
}

/// One `#undef` or `-U`.
#[derive(Clone, Copy, Debug, Snap)]
pub struct Undef {
    /// The directive's `#`.
    pub hash: Loc,
    /// The name token.
    pub name_loc: Loc,
    /// The name.
    pub name: Symbol,
    /// `File`, `Predefined` or `CommandLine`.
    pub source: DefSource,
    /// The definition it ended, if the name was defined.
    pub ended: Option<DefIx>,
}

/// A conditional directive's index in [`PpRecord::conds`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Snap)]
pub struct CondIx(u32);

/// An inclusion directive's index in [`PpRecord::incs`]; an entered one
/// also names its instance (the include context of what lies in it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Snap)]
pub struct IncIx(u32);

impl CondIx {
    /// The index.
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// The row at position `i` of [`PpRecord::conds`].
    ///
    /// # Panics
    /// If `i` does not fit in a `u32`.
    pub fn from_index(i: usize) -> CondIx {
        CondIx(u32::try_from(i).expect("a row index"))
    }
}

impl IncIx {
    /// The index.
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// The row at position `i` of [`PpRecord::incs`].
    ///
    /// # Panics
    /// If `i` does not fit in a `u32`.
    pub fn from_index(i: usize) -> IncIx {
        IncIx(u32::try_from(i).expect("a row index"))
    }
}

/// The kind of a conditional directive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondKind {
    /// `#if`.
    If,
    /// `#ifdef`.
    Ifdef,
    /// `#ifndef`.
    Ifndef,
    /// `#elif`.
    Elif,
    /// `#elifdef`.
    Elifdef,
    /// `#elifndef`.
    Elifndef,
    /// `#else`.
    Else,
    /// `#endif`.
    Endif,
}

impl CondKind {
    const ALL: [CondKind; 8] = [
        CondKind::If,
        CondKind::Ifdef,
        CondKind::Ifndef,
        CondKind::Elif,
        CondKind::Elifdef,
        CondKind::Elifndef,
        CondKind::Else,
        CondKind::Endif,
    ];

    /// `#if`, `#ifdef` or `#ifndef`: it opens a conditional.
    pub const fn is_opener(self) -> bool {
        matches!(self, CondKind::If | CondKind::Ifdef | CondKind::Ifndef)
    }

    /// The directive name.
    pub const fn name(self) -> &'static str {
        match self {
            CondKind::If => "if",
            CondKind::Ifdef => "ifdef",
            CondKind::Ifndef => "ifndef",
            CondKind::Elif => "elif",
            CondKind::Elifdef => "elifdef",
            CondKind::Elifndef => "elifndef",
            CondKind::Else => "else",
            CondKind::Endif => "endif",
        }
    }
}

/// What became of a conditional directive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondOutcome {
    /// Its condition was evaluated true (`#else`: no sibling was taken):
    /// its group was preprocessed.
    Taken,
    /// Its condition was evaluated false (or was erroneous), or an `#else`
    /// whose sibling was taken: its group was skipped.
    NotTaken,
    /// An `#elif*` after a taken group: its condition was never evaluated.
    NotEvaluated,
    /// A directive of a conditional nested in a skipped group: nesting
    /// only, nothing evaluated.
    Dead,
    /// A live `#endif`.
    Closes,
}

impl CondOutcome {
    const ALL: [CondOutcome; 5] = [
        CondOutcome::Taken,
        CondOutcome::NotTaken,
        CondOutcome::NotEvaluated,
        CondOutcome::Dead,
        CondOutcome::Closes,
    ];

    fn name(self) -> &'static str {
        match self {
            CondOutcome::Taken => "taken",
            CondOutcome::NotTaken => "not-taken",
            CondOutcome::NotEvaluated => "not-evaluated",
            CondOutcome::Dead => "dead",
            CondOutcome::Closes => "closes",
        }
    }
}

/// The bit of [`Conditional::opener`] that marks an include guard, and of
/// [`Conditional::next`] that marks an unterminated conditional.
const TOP_BIT: u32 = 1 << 31;

/// One conditional directive (20 bytes).
#[derive(Clone, Copy, Debug, Snap)]
pub struct Conditional {
    /// The directive's `#`: its key.
    pub hash: Loc,
    /// The end of the directive name: the condition is `cond..end`.
    pub cond: Loc,
    /// The end of the directive's logical line: its newline (past line
    /// splices and a block comment that continues on later lines), or the
    /// end of the file.
    pub end: Loc,
    /// `CondIx` of the opener (`IX_MASK`: none), kind in bits 28..31,
    /// `TOP_BIT`: an include guard kiln recognized.
    opener: u32,
    /// `CondIx` of the next directive of the conditional (`IX_MASK`: none
    /// yet), outcome in bits 28..31, `TOP_BIT` (on an opener): the file
    /// ended inside the conditional.
    next: u32,
}

impl Conditional {
    /// The directive's kind.
    pub fn kind(&self) -> CondKind {
        CondKind::ALL[((self.opener >> 28) & 7) as usize]
    }

    /// What became of it.
    pub fn outcome(&self) -> CondOutcome {
        CondOutcome::ALL.get(((self.next >> 28) & 7) as usize).copied().unwrap_or(CondOutcome::Dead)
    }

    /// The `#if`/`#ifdef`/`#ifndef` of its conditional (itself for an
    /// opener; `None` for a stray `#elif`/`#else`/`#endif`).
    pub fn opener(&self) -> Option<CondIx> {
        let o = self.opener & IX_MASK;
        (o != IX_MASK).then_some(CondIx(o))
    }

    /// The next `#elif*`/`#else`/`#endif` of its conditional (`None` for an
    /// `#endif`, and for the last directive of a conditional the file
    /// ended in, or that is still open).
    pub fn next(&self) -> Option<CondIx> {
        let n = self.next & IX_MASK;
        (n != IX_MASK).then_some(CondIx(n))
    }

    /// On an opener: kiln recognized it as the file's include guard (its
    /// controlling macro decides whether a later `#include` is skipped).
    /// Final once the inclusion holding it has ended.
    pub fn is_guard(&self) -> bool {
        self.opener & TOP_BIT != 0
    }

    /// On an opener: the file ended inside the conditional (an error).
    pub fn is_unterminated(&self) -> bool {
        self.next & TOP_BIT != 0
    }

    /// From the `#` to the end of the directive's logical line.
    pub fn directive(&self) -> Span {
        Span::new(self.hash, self.end)
    }

    /// What follows the directive name to the end of the logical line,
    /// untrimmed (comments included; empty for a bare `#else`).
    pub fn condition(&self) -> Span {
        Span::new(self.cond, self.end)
    }
}

/// The kind of an inclusion directive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Snap)]
pub enum IncKind {
    /// `#include`.
    Include,
    /// `#include_next`.
    IncludeNext,
    /// `#import`.
    Import,
    /// A header-unit `import <h>;` / `import "h";`.
    HeaderImport,
    /// `#embed`.
    Embed,
    /// `-include`: an `#include` line of the command-line part of the
    /// predefines buffer.
    CommandLine,
}

impl IncKind {
    fn name(self) -> &'static str {
        match self {
            IncKind::Include => "include",
            IncKind::IncludeNext => "include_next",
            IncKind::Import => "import",
            IncKind::HeaderImport => "header-import",
            IncKind::Embed => "embed",
            IncKind::CommandLine => "-include",
        }
    }
}

/// What became of an inclusion directive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Snap)]
pub enum IncOutcome {
    /// The file was entered; `base` is the location of its byte 0 in this
    /// inclusion (the instance's range).
    Entered {
        /// Byte 0 of the inclusion's range.
        base: Loc,
    },
    /// Not entered: `#pragma once`, or an earlier `#import` of the file.
    SkippedOnce,
    /// Not entered: kiln recognized the file's include guard and its
    /// controlling macro is defined, by `def`.
    SkippedGuard {
        /// The guard macro's definition current at the directive.
        def: DefIx,
    },
    /// No file found (an error, or `-MG`).
    NotFound,
    /// Entering the file would pass the include depth limit (an error).
    DepthLimit,
    /// The depth limit was hit earlier and the file had already been
    /// entered once: it is not entered again (clang's include-cycle cut).
    AfterDepthLimit,
    /// No usable header name: nothing was looked up (an error).
    Malformed,
    /// An `#embed` whose resource was found and embedded.
    Embedded,
    /// A header-unit import whose unit was not read from source here: its
    /// macros came from a module file (the record is then incomplete,
    /// [`RecordFlags::IMPORTED`]) or from an earlier import in this
    /// translation unit. When kiln replays the copy of the header a module
    /// file carries, that copy's range is this row's instance (its rows
    /// have this context), not a range of `file`.
    Imported,
}

impl IncOutcome {
    fn name(self) -> &'static str {
        match self {
            IncOutcome::Entered { .. } => "entered",
            IncOutcome::SkippedOnce => "skipped-once",
            IncOutcome::SkippedGuard { .. } => "skipped-guard",
            IncOutcome::NotFound => "not-found",
            IncOutcome::DepthLimit => "depth-limit",
            IncOutcome::AfterDepthLimit => "after-depth-limit",
            IncOutcome::Malformed => "malformed",
            IncOutcome::Embedded => "embedded",
            IncOutcome::Imported => "imported",
        }
    }
}

/// One inclusion directive.
#[derive(Clone, Debug, Snap)]
pub struct Inclusion {
    /// The directive's `#` (the first token, `import` or `export`, of a
    /// header-unit import): its key.
    pub hash: Loc,
    /// The end of the directive's logical line (the end of the `;` of a
    /// header-unit import).
    pub end: Loc,
    /// The header name as written in the file: for a name from a macro, the
    /// invocation (the token `H` of `#include H`), never a macro location;
    /// for a malformed directive, the token that ended it.
    pub name: Span,
    /// The name with its delimiters, after macro expansion (`<vector>`,
    /// `"x.h"`); empty for a malformed directive.
    pub written: Box<str>,
    /// The file found (see spec/pp.md for which outcomes have one).
    pub file: Option<FileId>,
    /// The search-path index it was found at (`None`: relative to the
    /// includer, or an absolute name, or no file). For an `#embed`, the
    /// index in the `--embed-dir` list.
    pub lookup: Option<u32>,
    /// The directive's kind.
    pub kind: IncKind,
    /// Where the directive is: `File`, `Predefined` or `CommandLine`.
    pub source: DefSource,
    /// What became of it.
    pub outcome: IncOutcome,
    /// The first expansion entered while reading the name (`NONE`).
    name_exp: u32,
    /// The entered inclusion holding the directive (`NONE`: the main
    /// file, the predefines).
    parent: u32,
}

impl Inclusion {
    /// The entered inclusion whose range holds the directive (`None`: the
    /// main file or the predefines buffer).
    pub fn parent(&self) -> Option<IncIx> {
        (self.parent != NONE).then_some(IncIx(self.parent))
    }

    /// The name came from a macro: the first expansion entered while it
    /// was read (the name's expansions are the `IN_DIRECTIVE` rows from it
    /// whose name lies in [`Inclusion::name`]).
    pub fn name_exp(&self) -> Option<ExpIx> {
        (self.name_exp != NONE).then_some(ExpIx(self.name_exp))
    }

    /// `<...>` (else `"..."`); `None` for a malformed directive.
    pub fn angled(&self) -> Option<bool> {
        match self.written.as_bytes().first() {
            Some(b'<') => Some(true),
            Some(b'"') => Some(false),
            _ => None,
        }
    }
}

/// The include context of a location ([`PpRecord::inclusion_of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncContext {
    /// The main file (the root of every chain).
    Main,
    /// The predefines buffer `<built-in>` (predefines, `-D`, `-U`, the
    /// `-include` lines).
    Builtin,
    /// The inclusion (instance) whose range holds it: an `Entered` row,
    /// or an `Imported` one whose module file's copy was replayed.
    Inclusion(IncIx),
    /// A location of a declaration imported from a module image: the
    /// buffer is one of [`PpRecord::images`]`[image]`'s files, which this
    /// translation unit did not preprocess (not recorded,
    /// [`RecordFlags::IMPORTED`]).
    Imported {
        /// The index in [`PpRecord::images`].
        image: u32,
    },
    /// A location the record did not see entered: invalid, unknown, or in
    /// a buffer lexed in its own range that is not the main file (a
    /// scratch buffer, a named module's replayed unit).
    Unknown,
}

/// An import whose unit was built from its module file's declaration
/// image (spec/pp.md "Preprocessing record", stage 4): its purview was not
/// preprocessed here, so the record has no rows for it; a location in one
/// of its `files` answers [`Origin::Imported`] (a macro location) or
/// [`IncContext::Imported`].
#[derive(Clone, Debug, PartialEq, Eq, Snap)]
pub struct ImageUnit {
    /// The module (`M`, `M:P`).
    pub module: Box<str>,
    /// The module name of the import.
    pub at: Loc,
    /// The buffers the image's locations are in (its file table, the
    /// unit's main file first).
    pub files: Box<[FileId]>,
}

/// What made a token spelled in scratch space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MadeKind {
    /// `##`.
    Paste,
    /// `#` (or `#__VA_OPT__`).
    Stringize,
    /// A builtin macro's result.
    Builtin,
    /// The text of a `_Pragma` operand.
    PragmaText,
    /// A macro-expanded `<...>` glued into a header name.
    HeaderName,
    /// The `##` of a paste that formed no valid token (for its diagnostic).
    InvalidPaste,
    /// The bytes of an `#embed` directive (WP-2544): `exp` is `None` and
    /// `from` the directive's `#`.
    Embed,
}

/// A kind of entry row (the top 4 bits of the row's `u32`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Body = 0,
    Single = 1,
    Paste = 2,
    Stringize = 3,
    Builtin = 4,
    PragmaText = 5,
    HeaderName = 6,
    InvalidPaste = 7,
}

impl EntryKind {
    fn from_bits(b: u32) -> EntryKind {
        match b {
            0 => EntryKind::Body,
            1 => EntryKind::Single,
            2 => EntryKind::Paste,
            3 => EntryKind::Stringize,
            4 => EntryKind::Builtin,
            5 => EntryKind::PragmaText,
            6 => EntryKind::HeaderName,
            _ => EntryKind::InvalidPaste,
        }
    }

    fn made(self) -> Option<MadeKind> {
        Some(match self {
            EntryKind::Body | EntryKind::Single => return None,
            EntryKind::Paste => MadeKind::Paste,
            EntryKind::Stringize => MadeKind::Stringize,
            EntryKind::Builtin => MadeKind::Builtin,
            EntryKind::PragmaText => MadeKind::PragmaText,
            EntryKind::HeaderName => MadeKind::HeaderName,
            EntryKind::InvalidPaste => MadeKind::InvalidPaste,
        })
    }
}

/// Where a token location came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Not a macro location.
    File(Loc),
    /// Body token `tok` of `exp`'s definition.
    Body {
        /// The expansion.
        exp: ExpIx,
        /// The body token's index.
        tok: u32,
    },
    /// A token of argument `arg` of `exp`, substituted at body token
    /// `use_tok`.
    Arg {
        /// The expansion.
        exp: ExpIx,
        /// The argument's index.
        arg: u32,
        /// The body token (a parameter) it was substituted for.
        use_tok: u32,
        /// Its location before substitution (ask again if it is a macro
        /// location).
        from: Loc,
        /// Its index among the argument's tokens, when `from` is one.
        index: Option<u32>,
    },
    /// A token spelled in scratch space.
    Made {
        /// The expansion it was made in (`None` for a header name).
        exp: Option<ExpIx>,
        /// What made it.
        kind: MadeKind,
        /// The entry's expansion range (see spec/pp.md).
        from: Span,
        /// The body tokens of `exp`'s definition that `from` covers (the
        /// operands of a paste, the `#` and its operand), when it lies in
        /// the replacement list.
        body: Option<(u32, u32)>,
    },
    /// A macro location of a declaration imported from a module image,
    /// whose expansion location is in one of
    /// [`PpRecord::images`]`[image]`'s files: made when the module was
    /// compiled, not here (not recorded, [`RecordFlags::IMPORTED`]).
    Imported {
        /// The index in [`PpRecord::images`].
        image: u32,
    },
    /// A macro location the record did not see made.
    Unknown,
}

/// One use of a parameter in a replacement list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Use {
    /// The body token's index.
    pub tok: u32,
    /// How it is used.
    pub kind: ParamUse,
    /// Inside `__VA_OPT__( )`.
    pub in_va_opt: bool,
}

/// How a parameter is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamUse {
    /// Substituted (pre-expanded).
    Plain,
    /// Operand of `#` (or inside `#__VA_OPT__( )`).
    Stringized,
    /// Next to a `##` paste operator (substituted unexpanded).
    PasteOperand,
}

/// Gaps in what the record covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Snap)]
pub struct RecordFlags(u8);

impl RecordFlags {
    /// A PCH snapshot or a preprocessor-state record was installed, or a
    /// definition was met that the record did not see made.
    pub const RESTORED: RecordFlags = RecordFlags(1);
    /// A named module's unit was built from its declaration image (its
    /// purview was not preprocessed here, [`PpRecord::images`]), or a
    /// header unit's macros were met that the record did not see made.
    pub const IMPORTED: RecordFlags = RecordFlags(2);
    /// Location space ran out: an expansion or an inclusion has no
    /// locations of its own.
    pub const LOC_SPACE_EXHAUSTED: RecordFlags = RecordFlags(4);
    /// More expansions than the record can index (2^28).
    pub const TRUNCATED: RecordFlags = RecordFlags(8);

    /// No gap.
    pub const fn empty() -> RecordFlags {
        RecordFlags(0)
    }

    /// All of `f` are set.
    pub const fn contains(self, f: RecordFlags) -> bool {
        self.0 & f.0 == f.0
    }

    /// No flag is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The flags as bits.
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// The preprocessing record of one translation unit.
#[derive(Debug, Snap)]
pub struct PpRecord {
    defs: Vec<Definition>,
    exps: Vec<Expansion>,
    args: Vec<ArgRow>,
    arg_toks: Vec<Loc>,
    params: Vec<Loc>,
    /// Base of each Macro entry made in expansion, with
    /// `EntryKind << 28 | ExpIx` (`IX_MASK`: none); sorted by base.
    entries: Vec<(Loc, u32)>,
    undefs: Vec<Undef>,
    /// `Rc::as_ptr` of every recorded definition (the record keeps each
    /// alive, so an address is never reused while it is a key), and of
    /// every alias. Rebuilt after a snapshot restore.
    #[snap(skip)]
    by_ptr: FxHashMap<usize, DefIx>,
    /// A header unit's exported definitions as its importers see them
    /// (they went through the preprocessor-state record, spec/modules.md
    /// §11.13 "Header units"), each with the definition recorded in the
    /// unit: kept alive here, keyed in `by_ptr`.
    aliases: Vec<(Rc<MacroDef>, DefIx)>,
    /// The imports whose unit was built from a declaration image.
    images: Vec<ImageUnit>,
    flags: std::cell::Cell<RecordFlags>,
    /// Parameter locations of the `#define` being read.
    #[snap(skip)]
    pub(crate) pending_params: Vec<Loc>,
    /// The end of the `#define` being read (its line's newline).
    pub(crate) pending_end: Loc,
    /// The main predefines buffer and the offset where
    /// [`crate::PpOptions::defines`] start.
    pub(crate) predef_tail: Option<(FileId, u32)>,
    va_opt: Symbol,
    conds: Vec<Conditional>,
    incs: Vec<Inclusion>,
    /// Base of each entered inclusion's range with its `IncIx`; sorted by
    /// base.
    inc_bases: Vec<(Loc, u32)>,
    onces: Vec<Loc>,
    /// The resource of each `Embedded` row that has one, by `IncIx`
    /// ([`PpRecord::resource`]); sorted.
    resources: Vec<(u32, Box<str>)>,
    /// The main file (its own range is the root context).
    pub(crate) main_file: Option<FileId>,
    /// Stage 3: made-token operands, condition reads, queries, push/pop
    /// events, conditional nesting.
    s3: stage3::Stage3,
}

/// A conditional's kind and outcome bits.
const fn cond_bits(kind: CondKind, outcome: CondOutcome) -> (u32, u32) {
    ((kind as u32) << 28, (outcome as u32) << 28)
}

fn ptr(d: &Rc<MacroDef>) -> usize {
    Rc::as_ptr(d) as usize
}

impl PpRecord {
    pub(crate) fn new(va_opt: Symbol) -> PpRecord {
        PpRecord {
            defs: Vec::new(),
            exps: Vec::new(),
            args: Vec::new(),
            arg_toks: Vec::new(),
            params: Vec::new(),
            entries: Vec::new(),
            undefs: Vec::new(),
            by_ptr: FxHashMap::default(),
            aliases: Vec::new(),
            images: Vec::new(),
            flags: std::cell::Cell::new(RecordFlags::empty()),
            pending_params: Vec::new(),
            pending_end: Loc::INVALID,
            predef_tail: None,
            va_opt,
            conds: Vec::new(),
            incs: Vec::new(),
            inc_bases: Vec::new(),
            onces: Vec::new(),
            resources: Vec::new(),
            main_file: None,
            s3: stage3::Stage3::default(),
        }
    }

    /// A conditional directive; `prev` is the latest row of its
    /// conditional (`NONE` for an opener, or a stray directive), `outer`
    /// (for an opener) the row whose group holds it (`NONE`: none). An
    /// evaluated row takes the reads of the condition just evaluated.
    /// Returns its index (`NONE` once full).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_cond(
        &mut self,
        kind: CondKind,
        outcome: CondOutcome,
        hash: Loc,
        cond: Loc,
        end: Loc,
        prev: u32,
        outer: u32,
    ) -> u32 {
        let c = self.push_cond_row(kind, outcome, hash, cond, end, prev);
        self.attach_cond(c, kind, outcome, outer);
        c
    }

    fn push_cond_row(
        &mut self,
        kind: CondKind,
        outcome: CondOutcome,
        hash: Loc,
        cond: Loc,
        end: Loc,
        prev: u32,
    ) -> u32 {
        let Ok(c) = u32::try_from(self.conds.len()) else { return NONE };
        if c >= IX_MASK {
            self.set_flag(RecordFlags::TRUNCATED);
            return NONE;
        }
        let opener = if kind.is_opener() {
            c
        } else {
            match self.conds.get_mut(prev as usize) {
                Some(p) => {
                    p.next = (p.next & !IX_MASK) | c;
                    p.opener & IX_MASK
                }
                None => IX_MASK,
            }
        };
        let (k, o) = cond_bits(kind, outcome);
        self.conds.push(Conditional { hash, cond, end, opener: opener | k, next: IX_MASK | o });
        c
    }

    /// The opener of row `c` is the include guard kiln recognized.
    pub(crate) fn set_guard(&mut self, c: u32) {
        if let Some(o) = self.conds.get(c as usize).map(|x| x.opener & IX_MASK)
            && let Some(x) = self.conds.get_mut(o as usize)
        {
            x.opener |= TOP_BIT;
        }
    }

    /// The file ended inside the conditional whose latest row is `c`.
    pub(crate) fn set_unterminated(&mut self, c: u32) {
        if let Some(o) = self.conds.get(c as usize).map(|x| x.opener & IX_MASK)
            && let Some(x) = self.conds.get_mut(o as usize)
        {
            x.next |= TOP_BIT;
        }
    }

    /// An inclusion directive; returns its index (`NONE` once full).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn inclusion(
        &mut self,
        kind: IncKind,
        source: DefSource,
        hash: Loc,
        name: Span,
        written: &str,
        name_exp: u32,
        parent: u32,
    ) -> u32 {
        let Ok(i) = u32::try_from(self.incs.len()) else { return NONE };
        if i >= IX_MASK {
            self.set_flag(RecordFlags::TRUNCATED);
            return NONE;
        }
        let name_exp = if (name_exp as usize) < self.exps.len() { name_exp } else { NONE };
        self.incs.push(Inclusion {
            hash,
            end: Loc::INVALID,
            name,
            written: written.into(),
            file: None,
            lookup: None,
            kind,
            source,
            outcome: IncOutcome::Malformed,
            name_exp,
            parent,
        });
        i
    }

    /// The end of inclusion `i`'s directive.
    pub(crate) fn set_inc_end(&mut self, i: u32, end: Loc) {
        if let Some(x) = self.incs.get_mut(i as usize) {
            x.end = end;
        }
    }

    /// What became of inclusion `i`, with the file found and its
    /// search-path index. An `Entered` one becomes an instance.
    pub(crate) fn set_inc_outcome(
        &mut self,
        i: u32,
        outcome: IncOutcome,
        file: Option<FileId>,
        lookup: Option<u32>,
    ) {
        let Some(x) = self.incs.get_mut(i as usize) else { return };
        x.outcome = outcome;
        x.file = file;
        x.lookup = lookup;
        if let IncOutcome::Entered { base } = outcome {
            self.set_inc_base(i, base);
        }
    }

    /// Inclusion `i`'s instance starts at `base` (an `Entered` row, or an
    /// `Imported` one whose module file's copy of the header is replayed).
    pub(crate) fn set_inc_base(&mut self, i: u32, base: Loc) {
        // Bases are handed out in increasing order, except a buffer's own
        // range (location space exhausted, a header unit).
        let at = self.inc_bases.partition_point(|&(b, _)| b <= base);
        self.inc_bases.insert(at, (base, i));
    }

    /// Whether row `i` has an instance (a range of its own).
    fn has_instance(&self, i: u32) -> bool {
        match self.incs.get(i as usize).map(|x| x.outcome) {
            Some(IncOutcome::Entered { .. }) => true,
            Some(IncOutcome::Imported) => self.inc_bases.iter().any(|&(_, j)| j == i),
            _ => false,
        }
    }

    /// The `#pragma once` (or `_Pragma("once")`) at `at` took effect.
    pub(crate) fn once(&mut self, at: Loc) {
        self.onces.push(at);
    }

    /// The number of expansions so far (where a directive's name
    /// expansions start).
    pub(crate) fn exp_count(&self) -> u32 {
        u32::try_from(self.exps.len()).unwrap_or(NONE)
    }

    // ---- building (the preprocessor's hooks) ----

    pub(crate) fn set_flag(&self, f: RecordFlags) {
        self.flags.set(RecordFlags(self.flags.get().0 | f.0));
    }

    /// A definition made by a directive (or the predefines).
    pub(crate) fn define(
        &mut self,
        def: &Rc<MacroDef>,
        old: Option<&Rc<MacroDef>>,
        source: DefSource,
        hash: Loc,
        end: Loc,
    ) {
        let d = self.push_def(def, source, hash, end);
        if let Some(o) = old {
            let o = self.lookup(o);
            self.defs[o.index()].ended = DefEnd::Replaced(d);
        }
    }

    fn push_def(&mut self, def: &Rc<MacroDef>, source: DefSource, hash: Loc, end: Loc) -> DefIx {
        let d = DefIx(u32::try_from(self.defs.len()).unwrap_or(u32::MAX));
        let params = u32::try_from(self.params.len()).unwrap_or(u32::MAX);
        let n = def.params.len();
        if self.pending_params.len() == n {
            self.params.append(&mut self.pending_params);
        } else {
            self.pending_params.clear();
            self.params.extend(std::iter::repeat_n(Loc::INVALID, n));
        }
        self.defs.push(Definition {
            def: Rc::clone(def),
            source,
            hash,
            end,
            ended: DefEnd::Live,
            params,
        });
        self.by_ptr.insert(ptr(def), d);
        d
    }

    /// The index of `def`, registering a definition the record did not see
    /// made (a builtin, or one restored or imported).
    pub(crate) fn lookup(&mut self, def: &Rc<MacroDef>) -> DefIx {
        if let Some(&d) = self.by_ptr.get(&ptr(def)) {
            return d;
        }
        let source = if def.builtin.is_some() {
            DefSource::Builtin
        } else {
            self.set_flag(RecordFlags::RESTORED);
            DefSource::Restored
        };
        self.pending_params.clear();
        self.push_def(def, source, Loc::INVALID, Loc::INVALID)
    }

    /// Is `def` recorded?
    pub(crate) fn knows(&self, def: &Rc<MacroDef>) -> bool {
        self.by_ptr.contains_key(&ptr(def))
    }

    /// An `#undef` (or `-U`) of `name`; `old` the definition it removes.
    pub(crate) fn undef(
        &mut self,
        name: Symbol,
        hash: Loc,
        name_loc: Loc,
        source: DefSource,
        old: Option<&Rc<MacroDef>>,
    ) {
        let ended = old.map(|o| {
            let o = self.lookup(o);
            self.defs[o.index()].ended = DefEnd::Undef(hash);
            o
        });
        self.undefs.push(Undef { hash, name_loc, name, source, ended });
    }

    /// `pop_macro` at `at` (its event's `at`) made `restored` current,
    /// replacing `current`.
    pub(crate) fn pop(
        &mut self,
        at: Loc,
        restored: Option<&Rc<MacroDef>>,
        current: Option<&Rc<MacroDef>>,
    ) {
        if let Some(c) = current {
            let c = self.lookup(c);
            self.defs[c.index()].ended = DefEnd::Popped(at);
        }
        if let Some(r) = restored {
            let r = self.lookup(r);
            self.defs[r.index()].ended = DefEnd::Live;
        }
    }

    /// Enters an expansion; returns its index (`NONE` once full).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn expansion(
        &mut self,
        def: &Rc<MacroDef>,
        name: Loc,
        end: Loc,
        parent: u32,
        flags: ExpFlags,
    ) -> u32 {
        let Ok(e) = u32::try_from(self.exps.len()) else { return NONE };
        if e >= IX_MASK {
            self.set_flag(RecordFlags::TRUNCATED);
            return NONE;
        }
        let d = self.lookup(def);
        let parent = if parent == NONE { IX_MASK } else { parent & IX_MASK };
        self.exps.push(Expansion {
            def: d,
            name,
            end,
            entry: Loc::INVALID,
            parent: parent | (u32::from(flags.0) << 28),
            args: u32::try_from(self.args.len()).unwrap_or(u32::MAX),
        });
        e
    }

    /// One argument of the expansion entered last: its token locations and
    /// the end of the last (or the `,`/`)` for an empty one).
    pub(crate) fn arg(&mut self, toks: impl Iterator<Item = Loc>, hi: Loc) {
        let tok_start = u32::try_from(self.arg_toks.len()).unwrap_or(u32::MAX);
        self.arg_toks.extend(toks);
        self.args.push(ArgRow { tok_start, hi });
    }

    /// A Macro entry with base `base` was made for expansion `e`.
    pub(crate) fn entry(&mut self, base: Loc, kind: EntryKind, e: u32) {
        let ix = if e == NONE { IX_MASK } else { e & IX_MASK };
        if matches!(kind, EntryKind::Body | EntryKind::Single | EntryKind::Builtin)
            && let Some(x) = self.exps.get_mut(e as usize)
        {
            x.entry = base;
        }
        debug_assert!(self.entries.last().is_none_or(|&(b, _)| b < base));
        self.entries.push((base, ((kind as u32) << 28) | ix));
    }

    // ---- reading ----

    /// Every definition made while recording, in order.
    pub fn defs(&self) -> &[Definition] {
        &self.defs
    }

    /// Definition `d`.
    pub fn def(&self, d: DefIx) -> &Definition {
        &self.defs[d.index()]
    }

    /// The index of a definition, if recorded (for example the one
    /// [`crate::MacroTable::get`] returns now).
    pub fn def_of(&self, def: &MacroDef) -> Option<DefIx> {
        self.by_ptr.get(&(std::ptr::from_ref(def) as usize)).copied()
    }

    /// Every expansion, in preprocessing order.
    pub fn exps(&self) -> &[Expansion] {
        &self.exps
    }

    /// Expansion `e`.
    pub fn exp(&self, e: ExpIx) -> &Expansion {
        &self.exps[e.index()]
    }

    /// The expansion whose result (or pre-expanded argument) held `e`'s
    /// name.
    pub fn parent(&self, e: ExpIx) -> Option<ExpIx> {
        let p = self.exps[e.index()].parent & IX_MASK;
        (p != IX_MASK).then_some(ExpIx(p))
    }

    /// `e`'s flags.
    pub fn flags(&self, e: ExpIx) -> ExpFlags {
        ExpFlags((self.exps[e.index()].parent >> 28) as u8)
    }

    /// `e`'s arguments, as collected.
    pub fn args(&self, e: ExpIx) -> impl ExactSizeIterator<Item = Arg<'_>> + '_ {
        let lo = self.exps[e.index()].args as usize;
        let hi = self.exps.get(e.index() + 1).map_or(self.args.len(), |x| x.args as usize);
        (lo..hi).map(move |i| self.arg_at(i))
    }

    fn arg_at(&self, i: usize) -> Arg<'_> {
        let r = self.args[i];
        let end = self.args.get(i + 1).map_or(self.arg_toks.len(), |n| n.tok_start as usize);
        let toks = &self.arg_toks[r.tok_start as usize..end];
        let lo = toks.first().copied().unwrap_or(r.hi);
        Arg { span: Span::new(lo, r.hi), toks }
    }

    /// The locations of `d`'s parameter names.
    pub fn param_locs(&self, d: DefIx) -> &[Loc] {
        let x = &self.defs[d.index()];
        let lo = x.params as usize;
        &self.params[lo..lo + x.def.params.len()]
    }

    /// The uses of parameter `p` in `d`'s replacement list.
    pub fn param_uses(&self, d: DefIx, p: usize) -> Vec<Use> {
        let def = &self.defs[d.index()].def;
        crate::expand::param_uses(def, self.va_opt)
            .into_iter()
            .filter(|&(q, _)| q as usize == p)
            .map(|(_, u)| u)
            .collect()
    }

    /// The expansions of `d`, in order (a scan).
    pub fn expansions_of(&self, d: DefIx) -> impl Iterator<Item = ExpIx> + '_ {
        self.exps.iter().enumerate().filter(move |(_, x)| x.def == d).map(|(i, _)| ExpIx(i as u32))
    }

    /// Every `#undef` and `-U`, in order.
    pub fn undefs(&self) -> &[Undef] {
        &self.undefs
    }

    /// Every conditional directive, in the order met (live and dead).
    pub fn conds(&self) -> &[Conditional] {
        &self.conds
    }

    /// Conditional directive `c`.
    pub fn cond(&self, c: CondIx) -> &Conditional {
        &self.conds[c.index()]
    }

    /// The group `c` controls: from the end of its line to the `#` of the
    /// next directive of its conditional, or, when there is none yet, to
    /// the end of the inclusion holding it (unterminated). `None` for an
    /// `#endif` and a stray directive (no opener).
    pub fn group(&self, sm: &SourceManager, c: CondIx) -> Option<Span> {
        let x = &self.conds[c.index()];
        if x.kind() == CondKind::Endif || x.opener().is_none() {
            return None;
        }
        let hi = if let Some(n) = x.next() {
            self.conds[n.index()].hash
        } else {
            let (f, off) = sm.offset_of(x.end)?;
            let len = u32::try_from(sm.text(f).len()).ok()?;
            x.end.with_offset(len.checked_sub(off)?)
        };
        Some(Span::new(x.end, hi))
    }

    /// The conditional directives whose `#` lies in the file range `span`
    /// (a scan).
    pub fn conds_in(&self, span: Span) -> impl Iterator<Item = CondIx> + '_ {
        self.conds
            .iter()
            .enumerate()
            .filter(move |(_, x)| span.lo <= x.hash && x.hash < span.hi)
            .map(|(i, _)| CondIx(i as u32))
    }

    /// Every inclusion directive, in the order met.
    pub fn incs(&self) -> &[Inclusion] {
        &self.incs
    }

    /// Inclusion directive `i`.
    pub fn inc(&self, i: IncIx) -> &Inclusion {
        &self.incs[i.index()]
    }

    /// An `Embedded` row's resource: the path it was read from as the
    /// resolver spelled it (`None` for every other row, and for an
    /// `#embed` replayed from a module file, which logs the bytes alone).
    pub fn resource(&self, i: IncIx) -> Option<&str> {
        let at = self.resources.binary_search_by_key(&i.0, |r| r.0).ok()?;
        Some(&self.resources[at].1)
    }

    /// Row `i`'s resource ([`Self::resource`]).
    pub(crate) fn set_resource(&mut self, i: u32, path: &str) {
        if (i as usize) < self.incs.len() && self.resources.last().is_none_or(|r| r.0 < i) {
            self.resources.push((i, path.into()));
        }
    }

    /// The `#` of every `#pragma once` that took effect (the expansion
    /// location of a `_Pragma("once")`), in order.
    pub fn onces(&self) -> &[Loc] {
        &self.onces
    }

    /// The include context of `loc`: the entered inclusion whose range
    /// holds it. A macro location is resolved through its expansion
    /// location (the file position of the outermost invocation). O(log n).
    pub fn inclusion_of(&self, sm: &SourceManager, loc: Loc) -> IncContext {
        let l = sm.expansion_loc(loc);
        let Some((f, off)) = sm.offset_of(l) else { return IncContext::Unknown };
        let base = Loc::from_raw(l.to_raw() - off);
        let at = self.inc_bases.partition_point(|&(b, _)| b < base);
        if let Some(&(b, i)) = self.inc_bases.get(at)
            && b == base
        {
            return IncContext::Inclusion(IncIx(i));
        }
        if self.main_file == Some(f) && sm.loc(f, 0) == base {
            IncContext::Main
        } else if sm.is_virtual(f) && sm.file_name(f) == "<built-in>" {
            IncContext::Builtin
        } else if let Some(image) = self.image_of_file(f) {
            IncContext::Imported { image }
        } else {
            IncContext::Unknown
        }
    }

    /// The imports whose unit was built from a declaration image, in
    /// order (spec/pp.md "Preprocessing record", stage 4).
    pub fn images(&self) -> &[ImageUnit] {
        &self.images
    }

    /// The image (the last one, if several share the buffer) whose file
    /// table holds `f`.
    fn image_of_file(&self, f: FileId) -> Option<u32> {
        let i = self.images.iter().rposition(|m| m.files.contains(&f))?;
        u32::try_from(i).ok()
    }

    /// The include chain of instance `i`: `i`, its parent, and so on out
    /// to the main file (or the predefines). Its cross-TU key is the
    /// sequence of the rows' keys (spec/pp.md).
    pub fn context(&self, i: IncIx) -> impl Iterator<Item = IncIx> + '_ {
        // Parents come before their children, so the walk ends.
        std::iter::successors(Some(i), move |p| self.incs[p.index()].parent())
    }

    /// The instances of `file`, in order (a scan).
    pub fn instances_of(&self, file: FileId) -> impl Iterator<Item = IncIx> + '_ {
        (0..self.incs.len() as u32)
            .filter(move |&i| self.incs[i as usize].file == Some(file) && self.has_instance(i))
            .map(IncIx)
    }

    /// What the record does not cover (empty: complete).
    pub fn gaps(&self) -> RecordFlags {
        self.flags.get()
    }

    /// The record covers the whole translation unit.
    pub fn is_complete(&self) -> bool {
        self.flags.get().is_empty()
    }

    /// The bytes the record holds: its tables, and the definitions only it
    /// keeps alive.
    pub fn bytes(&self) -> usize {
        use std::mem::size_of;
        let mut n = self.defs.capacity() * size_of::<Definition>()
            + self.exps.capacity() * size_of::<Expansion>()
            + self.args.capacity() * size_of::<ArgRow>()
            + (self.arg_toks.capacity() + self.params.capacity()) * size_of::<Loc>()
            + self.entries.capacity() * size_of::<(Loc, u32)>()
            + self.undefs.capacity() * size_of::<Undef>()
            + self.by_ptr.capacity() * (size_of::<(usize, DefIx)>() + 1)
            + self.conds.capacity() * size_of::<Conditional>()
            + self.incs.capacity() * size_of::<Inclusion>()
            + self.incs.iter().map(|x| x.written.len()).sum::<usize>()
            + self
                .resources
                .iter()
                .map(|r| size_of::<(u32, Box<str>)>() + r.1.len())
                .sum::<usize>()
            + self.aliases.capacity() * size_of::<(Rc<MacroDef>, DefIx)>()
            + self
                .images
                .iter()
                .map(|m| {
                    size_of::<ImageUnit>() + m.module.len() + m.files.len() * size_of::<FileId>()
                })
                .sum::<usize>()
            + self.inc_bases.capacity() * size_of::<(Loc, u32)>()
            + self.onces.capacity() * size_of::<Loc>()
            + self.s3.bytes();
        for d in &self.defs {
            if Rc::strong_count(&d.def) == 1 {
                let m = &d.def;
                n += size_of::<MacroDef>()
                    + m.body.len() * size_of::<crate::tok::PpTok>()
                    + m.rel.len() * size_of::<(u32, u32)>()
                    + m.params.len() * size_of::<Symbol>();
            }
        }
        n
    }

    /// The entry row of the Macro entry based at `base`.
    fn row(&self, base: Loc) -> Option<(EntryKind, Option<ExpIx>)> {
        let i = self.entries.binary_search_by_key(&base, |&(b, _)| b).ok()?;
        let v = self.entries[i].1;
        let e = v & IX_MASK;
        Some((EntryKind::from_bits(v >> 28), (e != IX_MASK).then_some(ExpIx(e))))
    }

    /// The index of the body token of `e`'s definition at `loc` in its
    /// replacement-list entry.
    fn body_tok(&self, e: ExpIx, loc: Loc) -> Option<u32> {
        let x = &self.exps[e.index()];
        let off = loc.to_raw().checked_sub(x.entry.to_raw())?;
        let rel = &self.defs[x.def.index()].def.rel;
        let i = rel.partition_point(|&(lo, _)| lo <= off).checked_sub(1)?;
        (off < rel[i].1.max(rel[i].0 + 1)).then(|| u32::try_from(i).unwrap_or(u32::MAX))
    }

    /// The body tokens of `e`'s definition that a made token's `from`
    /// covers, if it lies in `e`'s replacement-list entry. A paste's range
    /// ends at the start of its last operand, a stringization's at the end
    /// of its operand (`expand.rs` `paste`, `stringify`).
    fn body_range(
        &self,
        sm: &SourceManager,
        e: ExpIx,
        kind: MadeKind,
        span: Span,
    ) -> Option<(u32, u32)> {
        let x = &self.exps[e.index()];
        if !x.entry.is_valid() || sm.entry_bounds(span.lo)?.0 != x.entry {
            return None;
        }
        let lo = self.body_tok(e, span.lo)?;
        let rel = &self.defs[x.def.index()].def.rel;
        let off = span.hi.to_raw().checked_sub(x.entry.to_raw())?;
        let hi = match kind {
            MadeKind::Paste => rel.partition_point(|&(l, _)| l <= off),
            _ => rel.partition_point(|&(l, _)| l < off),
        };
        let hi = u32::try_from(hi.checked_sub(1)?).ok()?;
        Some((lo, hi.max(lo)))
    }

    /// Where the token at `loc` came from. O(log n).
    pub fn origin(&self, sm: &SourceManager, loc: Loc) -> Origin {
        match self.origin_seen(sm, loc) {
            Origin::Unknown if !self.images.is_empty() => {
                let at = sm.expansion_loc(loc);
                match sm.offset_of(at).and_then(|(f, _)| self.image_of_file(f)) {
                    Some(image) => Origin::Imported { image },
                    None => Origin::Unknown,
                }
            }
            o => o,
        }
    }

    /// [`Self::origin`] from the tables alone.
    fn origin_seen(&self, sm: &SourceManager, loc: Loc) -> Origin {
        if !sm.is_macro_loc(loc) {
            return Origin::File(loc);
        }
        let Some(info) = sm.expansion_info(loc) else { return Origin::Unknown };
        match info.kind {
            ExpansionKind::Macro { .. } => {
                let Some((kind, e)) = self.row(info.base) else { return Origin::Unknown };
                match (kind.made(), e) {
                    (Some(kind), e) => {
                        let from = info.expansion;
                        let body = e.and_then(|e| self.body_range(sm, e, kind, from));
                        Origin::Made { exp: e, kind, from, body }
                    }
                    (None, Some(e)) if kind == EntryKind::Single => Origin::Body { exp: e, tok: 0 },
                    (None, Some(e)) => match self.body_tok(e, loc) {
                        Some(tok) => Origin::Body { exp: e, tok },
                        None => Origin::Unknown,
                    },
                    (None, None) => Origin::Unknown,
                }
            }
            ExpansionKind::MacroArg => {
                let param = info.expansion.lo;
                let Some(pi) = sm.expansion_info(param) else { return Origin::Unknown };
                let Some((EntryKind::Body, Some(e))) = self.row(pi.base) else {
                    return Origin::Unknown;
                };
                let Some(use_tok) = self.body_tok(e, param) else { return Origin::Unknown };
                let def = &self.defs[self.exps[e.index()].def.index()].def;
                let Some(arg) = def
                    .body
                    .get(use_tok as usize)
                    .and_then(crate::tok::PpTok::ident)
                    .and_then(|s| def.param_index(s))
                else {
                    return Origin::Unknown;
                };
                let from = sm.immediate_spelling_loc(loc);
                let index = self.args(e).nth(arg).and_then(|a| {
                    let i = match a.toks.binary_search(&from) {
                        Ok(i) => Some(i),
                        Err(_) => a.toks.iter().position(|&t| t == from),
                    };
                    i.and_then(|i| u32::try_from(i).ok())
                });
                Origin::Arg {
                    exp: e,
                    arg: u32::try_from(arg).unwrap_or(u32::MAX),
                    use_tok,
                    from,
                    index,
                }
            }
            ExpansionKind::Embed => {
                Origin::Made { exp: None, kind: MadeKind::Embed, from: info.expansion, body: None }
            }
        }
    }

    // ---- the dump ----

    /// The dump (spec/pp.md "Preprocessing record", `-fkiln-dump-pp-record`):
    /// one `tok` line per `(location, spelling)` of `tokens` at a macro
    /// location, then one line per entry.
    pub fn dump(
        &self,
        sm: &SourceManager,
        interner: &Interner,
        tokens: &[(Loc, String)],
    ) -> String {
        self.dump_with(sm, interner, tokens, DumpOptions::default())
    }

    /// [`Self::dump`] with `opts`.
    pub fn dump_with(
        &self,
        sm: &SourceManager,
        interner: &Interner,
        tokens: &[(Loc, String)],
        opts: DumpOptions,
    ) -> String {
        let mut d = Dumper::new(self, sm, interner);
        d.opts = opts;
        let mut out = String::new();
        for (loc, sp) in tokens {
            if sm.is_macro_loc(*loc) {
                let _ = writeln!(out, "tok {sp} {}", d.origin(*loc));
            }
        }
        d.write(&mut out);
        out
    }
}

/// How [`PpRecord::dump_with`] prints (the default is [`PpRecord::dump`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DumpOptions {
    /// A condition's reads in source order: by the place in the
    /// directive's line each is at (its file location, through every
    /// expansion), entry order among reads at one place. By default they
    /// are in entry order, where an `expanded` read of a builtin
    /// (`__has_include(M)`) follows those of the macros its operand
    /// expanded (`-fkiln-dump-pp-record=source-order`).
    pub reads_in_source_order: bool,
}

/// Keys and formatting for [`PpRecord::dump`].
struct Dumper<'r, 's> {
    opts: DumpOptions,
    r: &'r PpRecord,
    sm: &'s SourceManager,
    interner: &'s Interner,
    /// Each expansion's key.
    exp_keys: Vec<String>,
    /// The source of each `#undef` by its `#`.
    undef_src: FxHashMap<Loc, DefSource>,
    /// Each inclusion's instance number (from 1, in order; 0: not
    /// entered), or for a `-include` line, its ordinal among them.
    inst: Vec<u32>,
    /// The key of each `pop_macro` event by its `at`.
    pop_keys: FxHashMap<Loc, String>,
}

impl<'r, 's> Dumper<'r, 's> {
    fn new(r: &'r PpRecord, sm: &'s SourceManager, interner: &'s Interner) -> Self {
        let undef_src = r.undefs.iter().map(|u| (u.hash, u.source)).collect();
        let mut n_inst = 0;
        let inst = (0..r.incs.len() as u32)
            .map(|i| {
                if r.has_instance(i) {
                    n_inst += 1;
                    n_inst
                } else {
                    0
                }
            })
            .collect();
        let mut dumper = Dumper {
            opts: DumpOptions::default(),
            r,
            sm,
            interner,
            exp_keys: Vec::with_capacity(r.exps.len()),
            undef_src,
            inst,
            pop_keys: FxHashMap::default(),
        };
        // The nearest ancestor (or self) whose name is in a file, and the
        // count of expansions keyed under each.
        let mut anchor: Vec<u32> = Vec::with_capacity(r.exps.len());
        let mut count: Vec<u32> = vec![0; r.exps.len()];
        for (i, x) in r.exps.iter().enumerate() {
            let e = ExpIx(i as u32);
            if !sm.is_macro_loc(x.name) {
                anchor.push(i as u32);
                dumper.exp_keys.push(dumper.file_loc(x.name));
                continue;
            }
            // A builtin is entered after its operands: its parent may come
            // later only if the record is inconsistent; guard anyway.
            let a = r.parent(e).and_then(|p| anchor.get(p.index()).copied()).unwrap_or(NONE);
            anchor.push(a);
            if a == NONE {
                dumper.exp_keys.push(format!("?{i}"));
            } else {
                count[a as usize] += 1;
                let k = format!("{}/{}", dumper.exp_keys[a as usize], count[a as usize]);
                dumper.exp_keys.push(k);
            }
        }
        for ev in r.stack_events() {
            if ev.kind() == StackKind::Pop {
                let k = dumper.stack_key(ev);
                dumper.pop_keys.insert(ev.at, k);
            }
        }
        dumper
    }

    /// `name:offset` of a file location.
    fn file_loc(&self, loc: Loc) -> String {
        match self.sm.offset_of(loc) {
            Some((f, off)) => {
                let n = self.sm.file_name(f);
                let n = n.rsplit(['/', '\\']).next().unwrap_or(n);
                format!("{n}:{off}")
            }
            None => "?".to_string(),
        }
    }

    fn def_key(&self, d: DefIx) -> String {
        let x = &self.r.defs[d.index()];
        let name = self.interner.as_str(x.def.name);
        match x.source {
            DefSource::File => self.file_loc(x.def.name_span.lo),
            DefSource::Builtin => format!("builtin:{name}"),
            DefSource::Predefined => format!("predefined:{name}"),
            DefSource::CommandLine => format!("command-line:{name}"),
            DefSource::Restored => format!("restored:{name}"),
        }
    }

    /// A location: a file position, or the origin of a macro location.
    fn loc(&self, loc: Loc) -> String {
        if self.sm.is_macro_loc(loc) { self.origin(loc) } else { self.file_loc(loc) }
    }

    fn origin(&self, loc: Loc) -> String {
        let mut s = String::new();
        let mut loc = loc;
        // Each step is one entry closer to a file (bounded by the chain).
        for _ in 0..64 {
            match self.r.origin(self.sm, loc) {
                Origin::File(l) => {
                    s.push_str(&self.file_loc(l));
                    return s;
                }
                Origin::Body { exp, tok } => {
                    let _ = write!(s, "{}:b{tok}", self.exp_keys[exp.index()]);
                    return s;
                }
                Origin::Arg { exp, arg, use_tok, from, index } => {
                    let _ = write!(s, "{}:a{arg}@b{use_tok}", self.exp_keys[exp.index()]);
                    if let Some(i) = index {
                        let _ = write!(s, "[{i}]");
                    }
                    s.push('<');
                    loc = from;
                }
                Origin::Made { exp, kind, body, .. } => {
                    let k = match kind {
                        MadeKind::Paste => "paste",
                        MadeKind::Stringize => "str",
                        MadeKind::Builtin => "builtin",
                        MadeKind::PragmaText => "pragma",
                        MadeKind::HeaderName => "header-name",
                        MadeKind::InvalidPaste => "invalid-paste",
                        MadeKind::Embed => "embed",
                    };
                    match exp {
                        Some(e) => {
                            let _ = write!(s, "{}:{k}", self.exp_keys[e.index()]);
                            if let Some((a, b)) = body {
                                let _ = write!(s, "(b{a}..b{b})");
                            }
                        }
                        None => s.push_str(k),
                    }
                    return s;
                }
                Origin::Imported { image } => {
                    let m = self.r.images.get(image as usize).map_or("", |m| &*m.module);
                    let _ = write!(s, "imported:{m}");
                    return s;
                }
                Origin::Unknown => {
                    s.push('?');
                    return s;
                }
            }
        }
        s.push('?');
        s
    }

    fn tok_text(&self, t: &crate::tok::PpTok) -> String {
        let sp = self.sm.spelling_loc(t.tok.span.lo);
        let sh = self.sm.spelling_loc(t.tok.span.hi);
        if let Some(text) = self.sm.span_text(Span::new(sp, sh)) {
            return text.to_string();
        }
        match t.tok.kind {
            kiln_lex::TokKind::Identifier(s) => self.interner.as_str(s).to_string(),
            kiln_lex::TokKind::Punct(p) => p.as_str().to_string(),
            _ => "?".to_string(),
        }
    }

    fn write(&self, out: &mut String) {
        self.write_defs(out);
        self.write_undefs(out);
        self.write_stack(out);
        self.write_exps(out);
        self.write_mades(out);
        self.write_queries(out);
        self.write_incs(out);
        self.write_conds(out);
        self.write_onces(out);
        self.write_images(out);
        self.write_gaps(out);
    }

    /// ` in i<n>` when `loc` lies in an entered inclusion (nothing for the
    /// main file and the predefines).
    fn ctx(&self, out: &mut String, loc: Loc) {
        if let IncContext::Inclusion(i) = self.r.inclusion_of(self.sm, loc)
            && let Some(&n) = self.inst.get(i.index())
            && n > 0
        {
            let _ = write!(out, " in i{n}");
        }
    }

    /// The last path component of a header name with its delimiters.
    fn short_name(w: &str) -> String {
        let (open, inner, close) = match w.len() {
            2.. => (&w[..1], &w[1..w.len() - 1], &w[w.len() - 1..]),
            _ => return w.to_string(),
        };
        let inner = inner.rsplit(['/', '\\']).next().unwrap_or(inner);
        format!("{open}{inner}{close}")
    }

    fn inc_key(&self, i: usize) -> String {
        let x = &self.r.incs[i];
        if x.source == DefSource::File {
            return self.file_loc(x.hash);
        }
        let n = self.r.incs[..=i].iter().filter(|y| y.source == x.source).count();
        match x.source {
            DefSource::CommandLine => format!("command-line:{n}"),
            _ => format!("predefined:{n}"),
        }
    }

    fn write_incs(&self, out: &mut String) {
        let r = self.r;
        for (i, x) in r.incs.iter().enumerate() {
            let in_file = x.source == DefSource::File;
            let w = if in_file { x.written.to_string() } else { Self::short_name(&x.written) };
            let w = if w.is_empty() { "-" } else { &w };
            let _ = write!(out, "inc {} {} {w}", self.inc_key(i), x.kind.name());
            if let Some(e) = x.name_exp() {
                let _ = write!(out, " macro {}", self.exp_keys[e.index()]);
            }
            if in_file {
                let _ = write!(out, " name {}..{}", self.loc(x.name.lo), self.loc(x.name.hi));
            }
            let _ = write!(out, " {}", x.outcome.name());
            let res = r.resource(IncIx(i as u32));
            if let Some(n) = x.file.map(|f| self.sm.file_name(f)).or(res) {
                let _ = write!(out, " file {}", n.rsplit(['/', '\\']).next().unwrap_or(n));
            }
            if let Some(d) = x.lookup {
                let _ = write!(out, " dir {d}");
            }
            if let IncOutcome::SkippedGuard { def } = x.outcome {
                let _ = write!(out, " guard {}", self.def_key(def));
            }
            if let Some(&n) = self.inst.get(i)
                && n > 0
            {
                let _ = write!(out, " ctx i{n}");
            }
            if in_file {
                self.ctx(out, x.hash);
            }
            out.push('\n');
        }
    }

    fn write_conds(&self, out: &mut String) {
        let r = self.r;
        for (i, x) in r.conds.iter().enumerate() {
            // The predefines' own conditionals differ per clang version.
            if self.rec_builtin(x.hash) {
                continue;
            }
            let c = CondIx(i as u32);
            let _ = write!(
                out,
                "cond {} {} {}",
                self.file_loc(x.hash),
                x.kind().name(),
                x.outcome().name()
            );
            if x.is_guard() {
                out.push_str(" guard");
            }
            if x.is_unterminated() {
                out.push_str(" unterminated");
            }
            if !x.kind().is_opener() {
                match x.opener() {
                    Some(o) => {
                        let _ = write!(out, " open {}", self.file_loc(r.conds[o.index()].hash));
                    }
                    None => out.push_str(" stray"),
                }
            }
            let _ = write!(out, " cond {}..{}", self.file_loc(x.cond), self.file_loc(x.end));
            if let Some(g) = r.group(self.sm, c) {
                let _ = write!(out, " group {}..{}", self.file_loc(g.lo), self.file_loc(g.hi));
            }
            self.ctx(out, x.hash);
            out.push('\n');
            self.write_reads(out, c);
        }
    }

    fn write_onces(&self, out: &mut String) {
        for &at in &self.r.onces {
            let _ = write!(out, "once {}", self.file_loc(at));
            self.ctx(out, at);
            out.push('\n');
        }
    }

    /// `image <import> <module> files <main file>[ +<n>]`: an import built
    /// from its declaration image, and its buffers (the unit's main file
    /// and how many more).
    fn write_images(&self, out: &mut String) {
        for m in &self.r.images {
            let _ = write!(out, "image {} {}", self.file_loc(m.at), m.module);
            if let Some(&f) = m.files.first() {
                let n = self.sm.file_name(f);
                let _ = write!(out, " files {}", n.rsplit(['/', '\\']).next().unwrap_or(n));
            }
            if m.files.len() > 1 {
                let _ = write!(out, " +{}", m.files.len() - 1);
            }
            self.ctx(out, m.at);
            out.push('\n');
        }
    }

    /// `loc` is in the predefines buffer.
    fn rec_builtin(&self, loc: Loc) -> bool {
        self.r.inclusion_of(self.sm, loc) == IncContext::Builtin
    }

    fn write_defs(&self, out: &mut String) {
        let r = self.r;
        // Definitions not in a file: only when named (the driver adds
        // `-D`s of its own, per clang version).
        let mut named = vec![false; r.defs.len()];
        for x in &r.exps {
            named[x.def.index()] = true;
        }
        for u in &r.undefs {
            if let Some(d) = u.ended {
                named[d.index()] = true;
            }
        }
        for (i, x) in r.defs.iter().enumerate() {
            let d = DefIx(i as u32);
            let shown = x.source == DefSource::File || named[i];
            if !shown {
                continue;
            }
            let m = &x.def;
            let _ = write!(out, "def {} {}", self.def_key(d), self.interner.as_str(m.name));
            if m.function_like {
                out.push('(');
                for (k, &p) in m.params.iter().enumerate() {
                    if k > 0 {
                        out.push_str(", ");
                    }
                    if m.variadic && k + 1 == m.params.len() {
                        if m.named_variadic {
                            let _ = write!(out, "{}...", self.interner.as_str(p));
                        } else {
                            out.push_str("...");
                        }
                    } else {
                        out.push_str(self.interner.as_str(p));
                    }
                }
                out.push(')');
            }
            let src = match x.source {
                DefSource::File => "file",
                DefSource::Builtin => "builtin",
                DefSource::Predefined => "predefined",
                DefSource::CommandLine => "command-line",
                DefSource::Restored => "restored",
            };
            let _ = write!(out, " {src}");
            if x.source == DefSource::File && x.hash.is_valid() {
                let _ = write!(out, " dir {}..{}", self.file_loc(x.hash), self.file_loc(x.end));
            }
            if m.builtin.is_none() {
                out.push_str(" =");
                for t in &m.body {
                    let _ = write!(out, " {}", self.tok_text(t));
                }
            }
            match x.ended {
                DefEnd::Live => {}
                DefEnd::Undef(at) => {
                    let _ = write!(out, " ended undef {}", self.undef_key(at, m.name));
                }
                DefEnd::Replaced(n) => {
                    let _ = write!(out, " ended replaced {}", self.def_key(n));
                }
                DefEnd::Popped(at) => {
                    let k = self.pop_keys.get(&at).cloned().unwrap_or_else(|| self.loc(at));
                    let _ = write!(out, " ended popped {k}");
                }
            }
            if x.source == DefSource::File {
                self.ctx(out, m.name_span.lo);
            }
            out.push('\n');
            self.write_params(out, d);
        }
    }

    fn write_params(&self, out: &mut String, d: DefIx) {
        let r = self.r;
        let m = &r.defs[d.index()].def;
        let uses = crate::expand::param_uses(m, r.va_opt);
        for (k, &p) in m.params.iter().enumerate() {
            let at = r.param_locs(d).get(k).copied().unwrap_or(Loc::INVALID);
            let at = if at.is_valid() { self.loc(at) } else { "-".to_string() };
            let _ =
                write!(out, "param {} {k} {} {at} uses", self.def_key(d), self.interner.as_str(p));
            let mut any = false;
            for (_, u) in uses.iter().filter(|(q, _)| *q as usize == k) {
                any = true;
                let kind = match u.kind {
                    ParamUse::Plain => "plain",
                    ParamUse::Stringized => "str",
                    ParamUse::PasteOperand => "paste",
                };
                let va = if u.in_va_opt { "/va-opt" } else { "" };
                let _ = write!(out, " b{}:{kind}{va}", u.tok);
            }
            if !any {
                out.push_str(" none");
            }
            out.push('\n');
        }
    }

    fn write_undefs(&self, out: &mut String) {
        for u in &self.r.undefs {
            if u.source == DefSource::Predefined {
                continue;
            }
            let name = self.interner.as_str(u.name);
            let _ = write!(out, "undef {} {name} ends ", self.undef_key(u.hash, u.name));
            match u.ended {
                Some(d) => out.push_str(&self.def_key(d)),
                None => out.push_str("none"),
            }
            if u.source == DefSource::File {
                self.ctx(out, u.hash);
            }
            out.push('\n');
        }
    }

    fn write_exps(&self, out: &mut String) {
        let r = self.r;
        for (i, x) in r.exps.iter().enumerate() {
            let e = ExpIx(i as u32);
            let name = self.interner.as_str(r.defs[x.def.index()].def.name);
            let _ = write!(out, "exp {} {name} def {}", self.exp_keys[i], self.def_key(x.def));
            if self.sm.is_macro_loc(x.name) {
                let _ = write!(out, " name {}", self.origin(x.name));
            }
            if let Some(p) = r.parent(e) {
                let _ = write!(out, " parent {}", self.exp_keys[p.index()]);
            }
            let _ = write!(out, " end {}", self.loc(x.end));
            if !x.entry.is_valid() {
                out.push_str(" empty");
            }
            let f = r.flags(e);
            for (flag, s) in
                [(ExpFlags::IN_DIRECTIVE, " in-directive"), (ExpFlags::REGROUPED, " regrouped")]
            {
                if f.contains(flag) {
                    out.push_str(s);
                }
            }
            let args = r.args(e);
            if args.len() > 0 || f.contains(ExpFlags::VA_ELIDED) {
                out.push_str(" args");
                for (k, a) in args.enumerate() {
                    let _ = write!(
                        out,
                        " a{k}={}..{}/{}",
                        self.loc(a.span.lo),
                        self.loc(a.span.hi),
                        a.toks.len()
                    );
                }
                if f.contains(ExpFlags::VA_ELIDED) {
                    out.push_str(" elided");
                }
            }
            self.ctx(out, x.name);
            out.push('\n');
        }
    }

    fn write_gaps(&self, out: &mut String) {
        let g = self.r.gaps();
        if g.is_empty() {
            out.push_str("record complete\n");
        } else {
            out.push_str("record incomplete:");
            for (flag, s) in [
                (RecordFlags::RESTORED, " restored"),
                (RecordFlags::IMPORTED, " imported"),
                (RecordFlags::LOC_SPACE_EXHAUSTED, " loc-space-exhausted"),
                (RecordFlags::TRUNCATED, " truncated"),
            ] {
                if g.contains(flag) {
                    out.push_str(s);
                }
            }
            out.push('\n');
        }
    }

    fn undef_key(&self, hash: Loc, name: Symbol) -> String {
        match self.undef_src.get(&hash) {
            Some(DefSource::CommandLine) => format!("command-line:{}", self.interner.as_str(name)),
            Some(DefSource::Predefined) => format!("predefined:{}", self.interner.as_str(name)),
            _ => self.file_loc(hash),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArgRow, Conditional, Definition, Expansion, Inclusion, Read, StackEvent, Undef, stage3,
    };
    use kiln_source::Loc;
    use std::mem::size_of;

    /// Stage 4 (spec/pp.md "Preprocessing record"): every location of a
    /// module image's files, file or macro, answers `Imported`; a file
    /// the image does not list stays `Unknown`.
    #[test]
    fn image_locations_answer_imported() {
        use super::{IncContext, Origin, PpRecord, RecordFlags};
        use kiln_source::{ExpansionKind, SourceManager, Span};
        let sm = SourceManager::new();
        let interner = kiln_lex::Interner::new();
        let m = sm.add_virtual("m.cppm", "export module M;\nexport int n = X;\n".into()).unwrap();
        let other = sm.add_virtual("o.h", "int o;\n".into()).unwrap();
        let mut r = PpRecord::new(interner.intern("__VA_OPT__"));
        assert!(r.is_complete());
        r.image("M", sm.loc(other, 0), &[m]);
        assert!(r.gaps().contains(RecordFlags::IMPORTED));
        let at = sm.loc(m, 30);
        let name = Span::new(at, sm.loc(m, 31));
        let mac = sm.add_expansion(sm.loc(m, 0), 1, name, ExpansionKind::Macro { name }).unwrap();
        assert_eq!(r.inclusion_of(&sm, at), IncContext::Imported { image: 0 });
        assert_eq!(r.origin(&sm, mac), Origin::Imported { image: 0 });
        assert_eq!(r.inclusion_of(&sm, sm.loc(other, 2)), IncContext::Unknown);
        assert_eq!(r.images()[0].module.as_ref(), "M");
    }

    /// The per-entry costs spec/pp.md "Preprocessing record" states.
    #[test]
    fn entry_sizes() {
        assert_eq!(size_of::<Expansion>(), 24);
        assert_eq!(size_of::<ArgRow>(), 8);
        assert_eq!(size_of::<Loc>(), 4);
        assert_eq!(size_of::<Definition>(), 32);
        assert_eq!(size_of::<Undef>(), 24);
        assert_eq!(size_of::<(Loc, u32)>(), 8);
        assert_eq!(size_of::<Conditional>(), 20);
        assert_eq!(size_of::<Inclusion>(), 72);
        assert_eq!(size_of::<Read>(), 16);
        assert_eq!(size_of::<StackEvent>(), 20);
        assert_eq!(size_of::<stage3::QueryRow>(), 32);
        assert_eq!(size_of::<(u32, u32)>(), 8);
    }
}
