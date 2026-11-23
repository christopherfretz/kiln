//! Stage 3 of the preprocessing record (spec/pp.md "Preprocessing
//! record", WP-2112): the operands of made tokens, the reads of an
//! evaluated condition, the `__has_include` and feature queries, the
//! `push_macro`/`pop_macro` events, and the nesting of conditionals.

use std::fmt::Write as _;
use std::rc::Rc;

use kiln_lex::Symbol;
use kiln_snap::Snap;
use kiln_source::{FileId, Loc, SourceManager, Span};

use super::{
    CondIx, CondKind, CondOutcome, DefIx, Dumper, EntryKind, ExpIx, IX_MASK, MadeKind, NONE,
    Origin, PpRecord, RecordFlags,
};
use crate::macros::MacroDef;

/// The bit of a made row's start that marks a paste that formed no valid
/// token, and of a read list's start that marks a condition cut off by
/// an error.
const TOP: u32 = 1 << 31;
/// The bit of a made row's start that marks `#__VA_OPT__( )`.
const VA_OPT: u32 = 1 << 30;
/// The start index bits of a made row.
const START: u32 = VA_OPT - 1;

/// A token spelled in scratch space, with what made it
/// ([`PpRecord::made`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Made<'r> {
    /// The base of its source-manager entry (its key in the record).
    pub base: Loc,
    /// The expansion it was made in (`None` for a header name).
    pub exp: Option<ExpIx>,
    /// What made it.
    pub kind: MadeKind,
    /// The entry's expansion range, as [`Origin::Made`] gives it.
    pub from: Span,
    /// The body tokens of `exp`'s definition that `from` covers, as
    /// [`Origin::Made`] gives it: for a paste, from its first operand to its
    /// last, so `body.0` is where it sits in the replacement list.
    pub body: Option<(u32, u32)>,
    /// A paste: the location of each token joined, left to right, before
    /// pasting (`n + 1` for `n` `##`; for an invalid paste, the tokens
    /// joined so far and then the one rejected). `#__VA_OPT__( )`: the
    /// tokens stringized. Empty for the other kinds.
    pub operands: &'r [Loc],
    /// `#x`: the index of the argument stringized (`args(exp).nth(arg)`).
    pub arg: Option<u32>,
    /// `#__VA_OPT__( )` (its tokens are `operands`).
    pub va_opt: bool,
    /// A paste that formed no valid token (an error): the result is the
    /// tokens joined before the one rejected, which is delivered after it.
    pub invalid: bool,
}

/// What a read of a condition is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadKind {
    /// A macro name tested: `defined X`, `#ifdef X` and its relatives.
    Tested,
    /// A macro expanded while the condition was evaluated.
    Expanded(ExpIx),
    /// An identifier left after expansion (it evaluates to 0).
    Left,
}

/// One read of an evaluated condition (16 bytes).
#[derive(Clone, Copy, Debug, Snap)]
pub struct Read {
    /// `Tested` and `Left`: the identifier token read (a macro location
    /// when it came from a macro body or argument). `Expanded`: the
    /// expansion's name token.
    pub at: Loc,
    /// The macro name read.
    pub name: Symbol,
    /// The definition bound at that point (`NONE`: undefined).
    def: u32,
    /// Kind in bits 30..32, bit 29 reserved, the `ExpIx` of an
    /// `Expanded` read below.
    bits: u32,
}

const READ_TESTED: u32 = 0;
const READ_EXPANDED: u32 = 1 << 30;
const READ_LEFT: u32 = 2 << 30;
const READ_KIND: u32 = 3 << 30;

impl Read {
    /// What the read is.
    pub fn kind(&self) -> ReadKind {
        match self.bits & READ_KIND {
            READ_EXPANDED => ReadKind::Expanded(ExpIx(self.bits & IX_MASK)),
            READ_LEFT => ReadKind::Left,
            _ => ReadKind::Tested,
        }
    }

    /// The definition of `name` bound when it was read (`None`: the name
    /// was not a macro).
    pub fn def(&self) -> Option<DefIx> {
        (self.def != NONE).then_some(DefIx(self.def))
    }
}

/// A `__has_include`, `__has_embed` or feature query
/// ([`PpRecord::query`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query<'r> {
    /// The builtin's expansion (its definition says which builtin).
    pub exp: ExpIx,
    /// The value the query produced.
    pub value: u32,
    /// The operand: a header name with its delimiters after expansion
    /// (`<optional>`); an attribute name with its scope (`gnu::unused`);
    /// a warning option (`-Wfoo`); an identifier. Empty when malformed.
    pub operand: &'r str,
    /// The operand's file range (for an operand from a macro, the
    /// invocation's range, never a macro location); invalid when there is
    /// none.
    pub name: Span,
    /// `__has_include`: the file found.
    pub file: Option<FileId>,
    /// `__has_include`: the search-path index it was found at.
    pub lookup: Option<u32>,
    /// The header name was produced by macro expansion.
    pub from_macro: bool,
}

/// One query row (32 bytes).
#[derive(Clone, Copy, Debug, Snap)]
pub(super) struct QueryRow {
    exp: u32,
    value: u32,
    /// The end of the operand in `query_text`; it starts at the previous
    /// row's end.
    text: u32,
    name: Span,
    file: Option<FileId>,
    /// `NONE`, or the index; `TOP`: the name came from a macro.
    lookup: u32,
}

/// The query being evaluated (its row is made when its expansion is).
#[derive(Clone, Copy, Debug)]
pub(super) struct PendingQuery {
    name: Span,
    file: Option<FileId>,
    lookup: Option<u32>,
    from_macro: bool,
}

impl Default for PendingQuery {
    fn default() -> Self {
        PendingQuery { name: Span::INVALID, file: None, lookup: None, from_macro: false }
    }
}

/// What a `push_macro`/`pop_macro` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackKind {
    /// `push_macro`: `def` was saved.
    Push,
    /// `pop_macro`: `def` became current again, ending `replaced`.
    Pop,
    /// `pop_macro` with nothing pushed for the name: nothing changed.
    PopUnmatched,
}

/// One `push_macro` or `pop_macro` (20 bytes).
#[derive(Clone, Copy, Debug, Snap)]
pub struct StackEvent {
    /// The directive's `#`, or the `_Pragma` token (then `exp()` is its
    /// expansion, whose key is the event's key).
    pub at: Loc,
    /// The macro name.
    pub name: Symbol,
    def: u32,
    replaced: u32,
    /// The `_Pragma`'s `ExpIx` (`IX_MASK`: none), kind in bits 30..32.
    exp: u32,
}

impl StackEvent {
    /// What it did.
    pub fn kind(&self) -> StackKind {
        match self.exp >> 30 {
            0 => StackKind::Push,
            1 => StackKind::Pop,
            _ => StackKind::PopUnmatched,
        }
    }

    /// `Push`: the definition saved (`None`: the name was undefined).
    /// `Pop`: the definition made current again (`None`: the name becomes
    /// undefined). `PopUnmatched`: `None`.
    pub fn def(&self) -> Option<DefIx> {
        (self.def != NONE).then_some(DefIx(self.def))
    }

    /// `Pop`: the definition it ended (`None`: the name was undefined).
    pub fn replaced(&self) -> Option<DefIx> {
        (self.replaced != NONE).then_some(DefIx(self.replaced))
    }

    /// The `_Pragma` expansion it ran in, if any.
    pub fn exp(&self) -> Option<ExpIx> {
        let e = self.exp & IX_MASK;
        (e != IX_MASK).then_some(ExpIx(e))
    }
}

/// The reads of the condition being evaluated.
#[derive(Clone, Copy, Debug, Default, Snap)]
pub(super) struct ReadState {
    /// A condition is being evaluated: reads are logged.
    active: bool,
    /// The reads of a finished evaluation wait for their row.
    pending: bool,
    /// The evaluation stopped on an error before the end of the line.
    partial: bool,
    /// The first expansion not yet logged as `Expanded`.
    next_exp: u32,
    /// `reads[..done]` belong to rows.
    done: u32,
}

/// The stage-3 tables of a [`PpRecord`].
#[derive(Debug, Default, Snap)]
pub(super) struct Stage3 {
    /// Base of each made entry with operands, with its start in `ops`
    /// (`TOP`: invalid paste, `VA_OPT`); sorted by base.
    made: Vec<(Loc, u32)>,
    ops: Vec<Loc>,
    /// The operands of the paste or `#__VA_OPT__` being made.
    #[snap(skip)]
    pub(super) pending_ops: Vec<Loc>,
    reads: Vec<Read>,
    /// `(CondIx, start in reads | TOP for partial)`, in `CondIx` order.
    cond_reads: Vec<(u32, u32)>,
    read: ReadState,
    queries: Vec<QueryRow>,
    query_text: String,
    #[snap(skip)]
    query: Option<PendingQuery>,
    stack: Vec<StackEvent>,
    /// Per conditional row: the `CondIx` of the directive whose group
    /// holds its conditional (`NONE`: none in its instance).
    enclosing: Vec<u32>,
    /// `(group start, CondIx)` sorted by start, built on demand for
    /// `conds.len()` rows.
    #[snap(skip)]
    cond_index: std::cell::RefCell<(usize, Vec<(Loc, u32)>)>,
}

/// `NONE`, or an index below `n`.
fn def_or(v: u32, n: usize) -> bool {
    v == NONE || (v as usize) < n
}

impl Stage3 {
    /// The stage-3 part of a decoded record's check (stage 4,
    /// [`crate::Preprocessor::snap_restore_record`]), for a record with
    /// `nd` definitions, `ne` expansions and `nc` conditional rows.
    pub(super) fn check(
        &self,
        nd: usize,
        ne: usize,
        nc: usize,
        sym: &dyn Fn(Symbol) -> bool,
        file: &dyn Fn(FileId) -> bool,
    ) -> Result<(), &'static str> {
        let def = |d: u32| d == NONE || (d as usize) < nd;
        let ops_in_order =
            self.made.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 & START <= w[1].1 & START);
        if !ops_in_order
            || self.made.last().is_some_and(|m| (m.1 & START) as usize > self.ops.len())
        {
            return Err("the made rows are not sorted or their operands out of range");
        }
        for r in &self.reads {
            let exp = match r.bits & READ_KIND {
                READ_EXPANDED => ((r.bits & IX_MASK) as usize) < ne,
                READ_LEFT | READ_TESTED => true,
                _ => false,
            };
            if !exp || !def(r.def) || !sym(r.name) {
                return Err("a condition read out of range");
            }
        }
        let done = self.read.done as usize;
        let reads_in_order =
            self.cond_reads.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 & !TOP <= w[1].1 & !TOP);
        let last_in = self.cond_reads.last().is_none_or(|l| (l.1 & !TOP) as usize <= done);
        let rows_in = self.cond_reads.iter().all(|&(c, _)| (c as usize) < nc);
        let state = !self.read.active && !self.read.pending && done == self.reads.len();
        if !reads_in_order || !last_in || !rows_in || !state || self.read.next_exp as usize > ne {
            return Err("the condition reads are not in order or not at rest");
        }
        let mut text = 0usize;
        for q in &self.queries {
            let t = q.text as usize;
            let spelled = self.query_text.get(text..t).is_some();
            if (q.exp as usize) >= ne || !spelled || q.file.is_some_and(|f| !file(f)) {
                return Err("a query row out of range");
            }
            text = t;
        }
        for e in &self.stack {
            let x = e.exp & IX_MASK;
            if !sym(e.name)
                || !def(e.def)
                || !def(e.replaced)
                || (x != IX_MASK && x as usize >= ne)
                || e.exp >> 30 > 2
            {
                return Err("a push/pop event out of range");
            }
        }
        if self.enclosing.len() > nc || !self.enclosing.iter().all(|&c| def_or(c, nc)) {
            return Err("the conditional nesting out of range");
        }
        Ok(())
    }

    pub(super) fn bytes(&self) -> usize {
        use std::mem::size_of;
        self.made.capacity() * size_of::<(Loc, u32)>()
            + (self.ops.capacity() + self.pending_ops.capacity()) * size_of::<Loc>()
            + self.reads.capacity() * size_of::<Read>()
            + self.cond_reads.capacity() * size_of::<(u32, u32)>()
            + self.queries.capacity() * size_of::<QueryRow>()
            + self.query_text.capacity()
            + self.stack.capacity() * size_of::<StackEvent>()
            + self.enclosing.capacity() * size_of::<u32>()
            + self.cond_index.borrow().1.capacity() * size_of::<(Loc, u32)>()
    }
}

fn ix(d: Option<DefIx>) -> u32 {
    d.map_or(NONE, |d| d.0)
}

impl PpRecord {
    // ---- building: made tokens ----

    /// The operands of the paste or `#__VA_OPT__` being made start again.
    pub(crate) fn ops_clear(&mut self) {
        self.s3.pending_ops.clear();
    }

    /// One more operand of the paste or `#__VA_OPT__` being made.
    pub(crate) fn op(&mut self, loc: Loc) {
        self.s3.pending_ops.push(loc);
    }

    /// The made entry just recorded at `base` gets the pending operands
    /// (nothing when no entry was recorded there: location space ran out).
    pub(crate) fn made_ops(&mut self, base: Loc, invalid: bool, va_opt: bool) {
        let recorded = self.entries.last().is_some_and(|&(b, _)| b == base);
        let start = u32::try_from(self.s3.ops.len()).unwrap_or(u32::MAX);
        if !recorded || start >= START {
            if recorded {
                self.set_flag(RecordFlags::TRUNCATED);
            }
            self.s3.pending_ops.clear();
            return;
        }
        let bits = start | if invalid { TOP } else { 0 } | if va_opt { VA_OPT } else { 0 };
        let s = &mut self.s3;
        s.made.push((base, bits));
        s.ops.append(&mut s.pending_ops);
    }

    // ---- building: reads ----

    /// A condition starts being evaluated: its reads are logged until
    /// [`Self::end_reads`].
    pub(crate) fn begin_reads(&mut self) {
        let next_exp = self.exp_count();
        let s = &mut self.s3;
        s.reads.truncate(s.read.done as usize);
        s.read = ReadState { active: true, next_exp, done: s.read.done, ..ReadState::default() };
    }

    /// The expansions entered since the last read, as `Expanded` reads.
    fn flush_expanded(&mut self) {
        let n = self.exp_count();
        for e in self.s3.read.next_exp..n {
            let x = &self.exps[e as usize];
            let name = self.defs[x.def.index()].def.name;
            let r = Read { at: x.name, name, def: x.def.0, bits: READ_EXPANDED | e };
            self.s3.reads.push(r);
        }
        self.s3.read.next_exp = n;
    }

    /// A macro name tested (`left`: an identifier left after expansion) at
    /// `at`, bound to `def`.
    pub(crate) fn read(&mut self, left: bool, at: Loc, name: Symbol, def: Option<&Rc<MacroDef>>) {
        if !self.s3.read.active {
            return;
        }
        self.flush_expanded();
        let def = def.map_or(NONE, |d| self.lookup(d).0);
        let bits = if left { READ_LEFT } else { READ_TESTED } | IX_MASK;
        self.s3.reads.push(Read { at, name, def, bits });
    }

    /// The condition's evaluation ended (`partial`: on an error before the
    /// end of its line). Its reads wait for its row.
    pub(crate) fn end_reads(&mut self, partial: bool) {
        if !self.s3.read.active {
            return;
        }
        self.flush_expanded();
        let r = &mut self.s3.read;
        r.active = false;
        r.pending = true;
        r.partial = partial;
    }

    /// Conditional row `c` was pushed with `outcome`: it takes the pending
    /// reads if it was evaluated; `outer` is the row whose group holds an
    /// opener (`NONE`).
    pub(super) fn attach_cond(&mut self, c: u32, kind: CondKind, outcome: CondOutcome, outer: u32) {
        let enc = if kind.is_opener() {
            outer
        } else {
            match self.conds.get(c as usize).and_then(super::Conditional::opener) {
                Some(o) => self.s3.enclosing.get(o.index()).copied().unwrap_or(NONE),
                None => NONE,
            }
        };
        if c != NONE {
            self.s3.enclosing.push(enc);
        }
        let s = &mut self.s3;
        let r = s.read;
        if !(r.pending || r.active) {
            return;
        }
        let evaluated = matches!(outcome, CondOutcome::Taken | CondOutcome::NotTaken)
            && !matches!(kind, CondKind::Else | CondKind::Endif);
        let n = u32::try_from(s.reads.len()).unwrap_or(u32::MAX);
        if c != NONE && evaluated && (n > r.done || r.partial) {
            s.cond_reads.push((c, r.done | if r.partial { TOP } else { 0 }));
            s.read.done = n;
        } else {
            s.reads.truncate(r.done as usize);
        }
        s.read.pending = false;
        s.read.active = false;
        s.read.partial = false;
    }

    // ---- building: queries ----

    /// A query builtin starts: nothing of an earlier one is pending.
    pub(crate) fn query_begin(&mut self) {
        let end = self.s3.queries_text_end();
        self.s3.query_text.truncate(end);
        self.s3.query = Some(PendingQuery::default());
    }

    /// The query's operand text and its file range.
    pub(crate) fn query_operand(&mut self, text: &str, name: Span) {
        let end = self.s3.queries_text_end();
        self.s3.query_text.truncate(end);
        self.s3.query_text.push_str(text);
        // A query in the operand (expanded first) took the pending one.
        self.s3.query.get_or_insert_with(PendingQuery::default).name = name;
    }

    /// `__has_include`: what the lookup found; `from_macro`: the name came
    /// from macro expansion.
    pub(crate) fn query_file(
        &mut self,
        file: Option<FileId>,
        lookup: Option<u32>,
        from_macro: bool,
    ) {
        let q = self.s3.query.get_or_insert_with(PendingQuery::default);
        q.file = file;
        q.lookup = lookup;
        q.from_macro = from_macro;
    }

    /// The pending query's builtin was entered as expansion `e` with
    /// `value` (no row when it was not entered).
    pub(crate) fn query_end(&mut self, e: u32, value: u32) {
        let Some(q) = self.s3.query.take() else { return };
        let nexps = self.exps.len();
        let s = &mut self.s3;
        if e == NONE || e as usize >= nexps {
            let end = s.queries_text_end();
            s.query_text.truncate(end);
            return;
        }
        let Ok(text) = u32::try_from(s.query_text.len()) else { return };
        let lookup =
            q.lookup.map_or(NONE & !TOP, |l| l & !TOP) | if q.from_macro { TOP } else { 0 };
        s.queries.push(QueryRow { exp: e, value, text, name: q.name, file: q.file, lookup });
    }

    // ---- building: push/pop ----

    /// A `push_macro` (`pop`: `pop_macro`; `matched`: something was
    /// pushed) of `name` at `at` (in the `_Pragma` expansion `exp`), with
    /// the definition saved or restored and the one replaced.
    pub(crate) fn stack_event(
        &mut self,
        at: Loc,
        name: Symbol,
        kind: StackKind,
        def: Option<&Rc<MacroDef>>,
        replaced: Option<&Rc<MacroDef>>,
        exp: u32,
    ) {
        let def = def.map(|d| self.lookup(d));
        let replaced = replaced.map(|d| self.lookup(d));
        let k = match kind {
            StackKind::Push => 0,
            StackKind::Pop => 1,
            StackKind::PopUnmatched => 2,
        };
        let e = if (exp as usize) < self.exps.len() { exp } else { IX_MASK };
        self.s3.stack.push(StackEvent {
            at,
            name,
            def: ix(def),
            replaced: ix(replaced),
            exp: e | (k << 30),
        });
    }

    // ---- reading: made tokens ----

    /// The made token at `loc`: a location in a `Paste`, `Stringize`,
    /// `Builtin`, `PragmaText`, `HeaderName` or `InvalidPaste` entry (the
    /// ones [`Origin::Made`] describes), with its operands. `None` for any
    /// other location (follow [`Origin::Arg`]'s `from` to reach the token a
    /// substituted argument came from). O(log n).
    pub fn made(&self, sm: &SourceManager, loc: Loc) -> Option<Made<'_>> {
        let Origin::Made { exp, kind, from, body } = self.origin(sm, loc) else { return None };
        let base = sm.entry_bounds(loc)?.0;
        let mut m = Made {
            base,
            exp,
            kind,
            from,
            body,
            operands: &[],
            arg: None,
            va_opt: false,
            invalid: false,
        };
        if let Ok(i) = self.s3.made.binary_search_by_key(&base, |&(b, _)| b) {
            let bits = self.s3.made[i].1;
            let lo = (bits & START) as usize;
            let hi = self.s3.made.get(i + 1).map_or(self.s3.ops.len(), |n| (n.1 & START) as usize);
            m.operands = &self.s3.ops[lo..hi];
            m.invalid = bits & TOP != 0;
            m.va_opt = bits & VA_OPT != 0;
        } else if kind == MadeKind::Stringize
            && let (Some(e), Some((_, j))) = (exp, body)
        {
            let def = &self.defs[self.exps[e.index()].def.index()].def;
            m.arg = def
                .body
                .get(j as usize)
                .and_then(crate::tok::PpTok::ident)
                .and_then(|s| def.param_index(s))
                .and_then(|p| u32::try_from(p).ok());
        }
        Some(m)
    }

    /// The base of every made entry, in order (each answers
    /// [`Self::made`]).
    pub fn mades(&self) -> impl Iterator<Item = Loc> + '_ {
        self.entries.iter().filter(|&&(_, v)| is_made(v)).map(|&(b, _)| b)
    }

    /// The made entries of expansion `e`, in order (a scan).
    pub fn mades_of(&self, e: ExpIx) -> impl Iterator<Item = Loc> + '_ {
        self.entries
            .iter()
            .filter(move |&&(_, v)| is_made(v) && v & IX_MASK == e.0)
            .map(|&(b, _)| b)
    }

    /// The spelling of the made token at `loc` (its whole entry).
    pub fn made_text<'s>(&self, sm: &'s SourceManager, loc: Loc) -> Option<&'s str> {
        let (lo, hi) = sm.entry_bounds(loc)?;
        let sp = sm.spelling_loc(lo);
        sm.span_text(Span::new(sp, sp.with_offset(hi.to_raw() - lo.to_raw())))
    }

    /// The scratch buffer the made token at `loc` is spelled in. Within a
    /// translation unit, two made tokens have the same text exactly when
    /// they have the same buffer. `None` for a token not spelled in scratch
    /// space (a failed paste's left operand, an `InvalidPaste` `##`).
    pub fn made_buffer(&self, sm: &SourceManager, loc: Loc) -> Option<FileId> {
        let sp = sm.spelling_loc(loc);
        if !sm.is_written_in_scratch_space(sp) {
            return None;
        }
        sm.offset_of(sp).map(|(f, _)| f)
    }

    // ---- reading: conditions ----

    fn read_slot(&self, c: CondIx) -> Option<usize> {
        self.s3.cond_reads.binary_search_by_key(&c.0, |&(x, _)| x).ok()
    }

    /// The reads of conditional row `c`, in evaluation order: empty for a
    /// row whose condition was not evaluated (`NotEvaluated`, `Dead`,
    /// `#else`, `#endif`) and for a condition that read nothing.
    pub fn reads(&self, c: CondIx) -> &[Read] {
        let Some(i) = self.read_slot(c) else { return &[] };
        let lo = (self.s3.cond_reads[i].1 & !TOP) as usize;
        let hi = self
            .s3
            .cond_reads
            .get(i + 1)
            .map_or(self.s3.read.done as usize, |n| (n.1 & !TOP) as usize);
        &self.s3.reads[lo..hi]
    }

    /// The evaluation of `c`'s condition stopped on an error before the end
    /// of its line: its reads are the ones made up to the error.
    pub fn reads_partial(&self, c: CondIx) -> bool {
        self.read_slot(c).is_some_and(|i| self.s3.cond_reads[i].1 & TOP != 0)
    }

    /// The directive whose group holds conditional row `c`'s conditional,
    /// in the same instance (`None`: at the top level of its file).
    pub fn enclosing(&self, c: CondIx) -> Option<CondIx> {
        let e = *self.s3.enclosing.get(c.index())?;
        (e != NONE).then_some(CondIx(e))
    }

    /// The innermost conditional directive whose group holds `loc`, live or
    /// skipped; each instance of a header has its own rows. A macro location
    /// is resolved through its expansion location. `None` outside every
    /// group. O(log n) after a first call builds an index.
    pub fn cond_at(&self, sm: &SourceManager, loc: Loc) -> Option<CondIx> {
        let loc = sm.expansion_loc(loc);
        if !loc.is_valid() {
            return None;
        }
        let mut ix = self.s3.cond_index.borrow_mut();
        if ix.0 != self.conds.len() || ix.1.is_empty() && !self.conds.is_empty() {
            let mut v: Vec<(Loc, u32)> = (0..self.conds.len())
                .filter(|&i| {
                    let x = &self.conds[i];
                    x.kind() != CondKind::Endif && x.opener().is_some()
                })
                .map(|i| (self.conds[i].end, i as u32))
                .collect();
            v.sort_unstable();
            *ix = (self.conds.len(), v);
        }
        let at = ix.1.partition_point(|&(lo, _)| lo <= loc);
        let mut c = ix.1.get(at.checked_sub(1)?).map(|&(_, c)| CondIx(c));
        drop(ix);
        // Each step is one conditional further out (bounded by the rows).
        while let Some(x) = c {
            if self.group(sm, x).is_some_and(|g| g.lo <= loc && loc < g.hi) {
                return Some(x);
            }
            c = self.enclosing(x);
        }
        None
    }

    // ---- reading: queries ----

    fn query_at(&self, i: usize) -> Query<'_> {
        let r = self.s3.queries[i];
        let lo = i.checked_sub(1).map_or(0, |j| self.s3.queries[j].text as usize);
        let lookup = r.lookup & !TOP;
        Query {
            exp: ExpIx(r.exp),
            value: r.value,
            operand: self.s3.query_text.get(lo..r.text as usize).unwrap_or(""),
            name: r.name,
            file: r.file,
            lookup: (lookup != NONE & !TOP).then_some(lookup),
            from_macro: r.lookup & TOP != 0,
        }
    }

    /// Every query, in the order of their expansions.
    pub fn queries(&self) -> impl ExactSizeIterator<Item = Query<'_>> + '_ {
        (0..self.s3.queries.len()).map(|i| self.query_at(i))
    }

    /// The query made by expansion `e` (a `__has_include`,
    /// `__has_include_next`, `__has_embed` or feature-query builtin).
    /// O(log n).
    pub fn query(&self, e: ExpIx) -> Option<Query<'_>> {
        let i = self.s3.queries.binary_search_by_key(&e.0, |r| r.exp).ok()?;
        Some(self.query_at(i))
    }

    // ---- reading: push/pop ----

    /// Every `push_macro` and `pop_macro` that named a macro, in order.
    pub fn stack_events(&self) -> &[StackEvent] {
        &self.s3.stack
    }

    /// For the `Pop` at `stack_events()[i]`: the index of the `Push` it
    /// undid (`None`: pushed before recording, or not a `Pop`).
    pub fn push_of(&self, i: usize) -> Option<usize> {
        let ev = self.s3.stack.get(i)?;
        if ev.kind() != StackKind::Pop {
            return None;
        }
        let mut depth = 0u32;
        for j in (0..i).rev() {
            let x = &self.s3.stack[j];
            if x.name != ev.name {
                continue;
            }
            match x.kind() {
                StackKind::Pop => depth += 1,
                StackKind::Push if depth == 0 => return Some(j),
                StackKind::Push => depth -= 1,
                StackKind::PopUnmatched => {}
            }
        }
        None
    }
}

impl Stage3 {
    fn queries_text_end(&self) -> usize {
        self.queries.last().map_or(0, |r| r.text as usize)
    }
}

/// An entry row's value names a made kind.
fn is_made(v: u32) -> bool {
    EntryKind::from_bits(v >> 28).made().is_some()
}

// ---- the dump ----

impl Dumper<'_, '_> {
    /// A `push_macro`/`pop_macro` event's key: its `_Pragma` expansion's,
    /// or its `#`.
    pub(super) fn stack_key(&self, ev: &StackEvent) -> String {
        match ev.exp() {
            Some(e) => self.exp_keys[e.index()].clone(),
            None => self.file_loc(ev.at),
        }
    }

    fn def_or_undefined(&self, d: Option<DefIx>) -> String {
        d.map_or_else(|| "undefined".to_string(), |d| self.def_key(d))
    }

    /// `push <key> <name> saves <def>|undefined`, `pop <key> <name> restores
    /// <def>|undefined replaces <def>|undefined [push <key>]`, `pop <key>
    /// <name> unmatched`.
    pub(super) fn write_stack(&self, out: &mut String) {
        let r = self.r;
        for (i, ev) in r.stack_events().iter().enumerate() {
            // The predefines' own pushes differ per clang version.
            if self.rec_builtin(ev.at) {
                continue;
            }
            let key = self.stack_key(ev);
            let name = self.interner.as_str(ev.name);
            match ev.kind() {
                StackKind::Push => {
                    let d = self.def_or_undefined(ev.def());
                    let _ = write!(out, "push {key} {name} saves {d}");
                }
                StackKind::Pop => {
                    let d = self.def_or_undefined(ev.def());
                    let old = self.def_or_undefined(ev.replaced());
                    let _ = write!(out, "pop {key} {name} restores {d} replaces {old}");
                    if let Some(j) = r.push_of(i) {
                        let _ = write!(out, " push {}", self.stack_key(&r.stack_events()[j]));
                    }
                }
                StackKind::PopUnmatched => {
                    let _ = write!(out, "pop {key} {name} unmatched");
                }
            }
            self.ctx(out, ev.at);
            out.push('\n');
        }
    }

    /// `made <origin> <text> = <op> ## <op>... [invalid]` for a paste,
    /// `made <origin> <text> arg <i>` for `#x`, `made <origin> <text> va-opt
    /// <op>...` for `#__VA_OPT__`.
    pub(super) fn write_mades(&self, out: &mut String) {
        let r = self.r;
        for base in r.mades() {
            let Some(m) = r.made(self.sm, base) else { continue };
            if !matches!(m.kind, MadeKind::Paste | MadeKind::Stringize) {
                continue;
            }
            let text = r.made_text(self.sm, base).unwrap_or("?");
            let _ = write!(out, "made {} {text}", self.origin(base));
            if m.kind == MadeKind::Paste {
                out.push_str(" =");
                for (k, &op) in m.operands.iter().enumerate() {
                    if k > 0 {
                        out.push_str(" ##");
                    }
                    let _ = write!(out, " {}", self.loc(op));
                }
                if m.invalid {
                    out.push_str(" invalid");
                }
            } else if m.va_opt {
                out.push_str(" va-opt");
                for &op in m.operands {
                    let _ = write!(out, " {}", self.loc(op));
                }
            } else if let Some(a) = m.arg {
                let _ = write!(out, " arg {a}");
            }
            self.ctx(out, base);
            out.push('\n');
        }
    }

    /// `query <exp> <builtin> <operand> = <value> [name <lo>..<hi>] [macro]
    /// [file <name>] [dir <n>]`.
    pub(super) fn write_queries(&self, out: &mut String) {
        let r = self.r;
        for q in r.queries() {
            let x = r.exp(q.exp);
            if self.rec_builtin(x.name) {
                continue;
            }
            let b = self.interner.as_str(r.def(x.def).def.name);
            let op = if q.operand.is_empty() { "-" } else { q.operand };
            let _ = write!(out, "query {} {b} {op} = {}", self.exp_keys[q.exp.index()], q.value);
            if q.name.is_valid() && !self.sm.is_macro_loc(q.name.lo) {
                let _ =
                    write!(out, " name {}..{}", self.file_loc(q.name.lo), self.file_loc(q.name.hi));
            }
            if q.from_macro {
                out.push_str(" macro");
            }
            if let Some(f) = q.file {
                let n = self.sm.file_name(f);
                let _ = write!(out, " file {}", n.rsplit(['/', '\\']).next().unwrap_or(n));
            }
            if let Some(d) = q.lookup {
                let _ = write!(out, " dir {d}");
            }
            self.ctx(out, x.name);
            out.push('\n');
        }
    }

    /// `read <cond> tested|expanded|left <name> <def>|undefined at <loc>
    /// [exp <key>]` per read of row `c`, then `reads <cond> partial`.
    pub(super) fn write_reads(&self, out: &mut String, c: CondIx) {
        let r = self.r;
        let key = self.file_loc(r.cond(c).hash);
        let mut reads: Vec<&Read> = r.reads(c).iter().collect();
        if self.opts.reads_in_source_order {
            reads.sort_by_key(|rd| self.sm.offset_of(self.sm.expansion_loc(rd.at)));
        }
        for rd in reads {
            let name = self.interner.as_str(rd.name);
            let (k, exp) = match rd.kind() {
                ReadKind::Tested => ("tested", None),
                ReadKind::Expanded(e) => ("expanded", Some(e)),
                ReadKind::Left => ("left", None),
            };
            let d = self.def_or_undefined(rd.def());
            let at = if rd.at.is_valid() { self.loc(rd.at) } else { "-".to_string() };
            let _ = write!(out, "read {key} {k} {name} {d} at {at}");
            if let Some(e) = exp {
                let _ = write!(out, " exp {}", self.exp_keys[e.index()]);
            }
            self.ctx(out, r.cond(c).hash);
            out.push('\n');
        }
        if r.reads_partial(c) {
            let _ = write!(out, "reads {key} partial");
            self.ctx(out, r.cond(c).hash);
            out.push('\n');
        }
    }
}
