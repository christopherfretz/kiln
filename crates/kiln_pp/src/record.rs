//! The preprocessing record (spec/pp.md "Preprocessing record"): what
//! macro expansion did, kept on request ([`crate::PpOptions::pp_record`])
//! so that a tooling client can ask where a token location came from.
//!
//! Nothing is stored per token. The record keeps the definitions (with
//! their history), one row per expansion with its argument spans, and a
//! side index of the source-manager entries made in macro expansion;
//! [`PpRecord::origin`] reads a location's entry and finds it there.

use std::fmt::Write as _;
use std::rc::Rc;

use kiln_lex::{Interner, Symbol};
use kiln_source::fx::FxHashMap;
use kiln_source::{ExpansionKind, Loc, SourceManager, Span};

use crate::macros::MacroDef;

/// A definition's index in [`PpRecord::defs`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefIx(u32);

/// An expansion's index in [`PpRecord::exps`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
}

/// No expansion (a `u32` slot).
const NONE: u32 = u32::MAX;
/// Expansion indices fit below the flag bits of [`Expansion::parent`].
const IX_MASK: u32 = 0x0FFF_FFFF;

/// Where a definition came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefEnd {
    /// Still in effect (or made current again by `pop_macro`).
    Live,
    /// Ended by the `#undef` (or `-U`) whose `#` is at the location.
    Undef(Loc),
    /// Replaced by a redefinition.
    Replaced(DefIx),
    /// Replaced by the `pop_macro` at the location.
    Popped(Loc),
}

/// One definition.
#[derive(Debug)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug)]
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
#[derive(Clone, Copy, Debug)]
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
#[derive(Clone, Copy, Debug)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecordFlags(u8);

impl RecordFlags {
    /// A PCH snapshot or a preprocessor-state record was installed, or a
    /// definition was met that the record did not see made.
    pub const RESTORED: RecordFlags = RecordFlags(1);
    /// A header unit's macros came from a module file or an image.
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
#[derive(Debug)]
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
    /// alive, so an address is never reused while it is a key).
    by_ptr: FxHashMap<usize, DefIx>,
    flags: std::cell::Cell<RecordFlags>,
    /// Parameter locations of the `#define` being read.
    pub(crate) pending_params: Vec<Loc>,
    /// The end of the `#define` being read (its line's newline).
    pub(crate) pending_end: Loc,
    /// The main predefines buffer and the offset where
    /// [`crate::PpOptions::defines`] start.
    pub(crate) predef_tail: Option<(kiln_source::FileId, u32)>,
    va_opt: Symbol,
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
            flags: std::cell::Cell::new(RecordFlags::empty()),
            pending_params: Vec::new(),
            pending_end: Loc::INVALID,
            predef_tail: None,
            va_opt,
        }
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

    /// `pop_macro` at `at` made `restored` current, replacing `current`.
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
            + self.by_ptr.capacity() * (size_of::<(usize, DefIx)>() + 1);
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
        let d = Dumper::new(self, sm, interner);
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

/// Keys and formatting for [`PpRecord::dump`].
struct Dumper<'r, 's> {
    r: &'r PpRecord,
    sm: &'s SourceManager,
    interner: &'s Interner,
    /// Each expansion's key.
    exp_keys: Vec<String>,
    /// The source of each `#undef` by its `#`.
    undef_src: FxHashMap<Loc, DefSource>,
}

impl<'r, 's> Dumper<'r, 's> {
    fn new(r: &'r PpRecord, sm: &'s SourceManager, interner: &'s Interner) -> Self {
        let undef_src = r.undefs.iter().map(|u| (u.hash, u.source)).collect();
        let mut dumper =
            Dumper { r, sm, interner, exp_keys: Vec::with_capacity(r.exps.len()), undef_src };
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
        self.write_exps(out);
        self.write_gaps(out);
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
                    let _ = write!(out, " ended popped {}", self.loc(at));
                }
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
    use super::{ArgRow, Definition, Expansion, Undef};
    use kiln_source::Loc;
    use std::mem::size_of;

    /// The per-entry costs spec/pp.md "Preprocessing record" states.
    #[test]
    fn entry_sizes() {
        assert_eq!(size_of::<Expansion>(), 24);
        assert_eq!(size_of::<ArgRow>(), 8);
        assert_eq!(size_of::<Loc>(), 4);
        assert_eq!(size_of::<Definition>(), 32);
        assert_eq!(size_of::<Undef>(), 24);
        assert_eq!(size_of::<(Loc, u32)>(), 8);
    }
}
