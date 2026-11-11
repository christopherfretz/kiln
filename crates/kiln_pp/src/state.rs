//! The preprocessor-state record (spec/modules.md §11.12, spec/pp.md
//! §13): the macro table, include guards, `#pragma once` files,
//! `push_macro` stacks, poisoned identifiers, the diagnostic pragma
//! state and `__COUNTER__` at the end of a header, as a module-file
//! section ([`crate::bmi::SECTION_PP`]).
//!
//! The writer snapshots a [`Preprocessor`] against a [`Baseline`] (the
//! state before the header was read) into a [`PpState`] and encodes it;
//! the reader decodes a [`PpRecord`], materializes it into an importer's
//! source manager (lazy buffers for the files, the location records of
//! `kiln_source::image_locs`) and installs it into a fresh
//! preprocessor, which then goes on as if it had read the header.
//! Macro bodies are tokens: nothing is lexed on load, and a body
//! token's spelling is read from its file only when it is printed.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;
use std::rc::Rc;

use kiln_diag::{DiagId, PragmaSnapshot, Severity};
use kiln_lex::{Encoding, Interner, Punct, Symbol, TokFlags, TokKind, Token};
use kiln_source::image_locs::{self, FileRec, LocTable, Locs, NONE, Reader, SlocRec, W};
use kiln_source::{FileId, LazyFile, Loc, SourceManager, Span};

use crate::Preprocessor;
use crate::builtin::Builtin;
use crate::macros::{AttrMacro, MacroDef, SymMap};
use crate::preprocessor::Frame;
use crate::tok::{PpFlags, PpTok};

/// The record's layout version (the first `u32` of the section).
pub const VERSION: u32 = 1;

/// Sema's pragma state at the end of the header, carried by the record
/// for the importer's Sema (spec/modules.md §11.12 "Sema pragma
/// state"); the driver fills it from the parse.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemaPragmas {
    /// The `#pragma pack` value in effect (0: none).
    pub pack: u16,
    /// The `#pragma pack(push)` entries: `(label, value)`, innermost last.
    pub pack_stack: Vec<(Option<String>, u16)>,
    /// `#pragma ms_struct on` is in effect.
    pub ms_struct: bool,
    /// The `#pragma GCC visibility push` entries (`None`: a
    /// `visibility` attribute push), innermost last: `(code, location)`
    /// with 0 default, 1 hidden, 2 protected.
    pub visibility: Vec<Option<(u32, Loc)>>,
    /// The FP pragma override in effect and the `float_control(push)`
    /// entries, in sema's bit encoding (`kiln_sema::fp_bits`).
    pub fp: u64,
    pub fp_stack: Vec<u64>,
}

/// The state a record is the difference from: the preprocessor at the
/// point the header begins (the predefines read, no `-include` yet).
#[derive(Debug, Default)]
pub struct Baseline {
    macros: SymMap<Rc<MacroDef>>,
    pushed: SymMap<Vec<Option<Rc<MacroDef>>>>,
    poisoned: HashSet<Symbol>,
    guards: HashSet<FileId>,
    once: HashSet<FileId>,
    attr_len: usize,
}

/// The preprocessor state a header leaves, as the difference from a
/// [`Baseline`]: what the record holds, either taken from a live
/// preprocessor ([`Preprocessor::state_snapshot`]) or materialized from a
/// record ([`PpRecord::materialize`]).
#[derive(Debug, Default)]
pub struct PpState {
    /// Definitions made or replaced, by name.
    pub(crate) macros: Vec<Rc<MacroDef>>,
    /// Baseline macros no longer defined.
    pub(crate) undefs: Vec<Symbol>,
    /// `push_macro` stacks that changed (`None`: pushed while undefined).
    pub(crate) pushed: Vec<(Symbol, Vec<Option<Rc<MacroDef>>>)>,
    pub(crate) poisoned: Vec<(Symbol, Span)>,
    /// Files with a detected include guard, and its macro.
    pub(crate) guards: Vec<(FileId, Symbol)>,
    pub(crate) once: Vec<FileId>,
    pub(crate) counter: u32,
    /// `#pragma clang diagnostic push`es not popped.
    pub(crate) diag_pushes: u32,
    /// Attribute-macro definitions made by the header (oldest first).
    pub(crate) attr: Vec<AttrMacro>,
    /// The `#pragma clang diagnostic` state.
    pub(crate) diag: PragmaSnapshot,
    /// Sema's pragma state (written by the producer; not compared by
    /// [`PpState::describe`]).
    pub sema: SemaPragmas,
}

/// Where a preprocessor takes a [`Baseline`], installs a record or
/// takes a checkpoint: at "points" of the predefines buffer, point `k`
/// being its `k`-th `#include` (from 0) as it begins, or the end of the
/// buffer (spec/pp.md §13).
#[derive(Debug, Default)]
pub struct StateHooks {
    /// Take the baseline at point 0.
    pub baseline: bool,
    /// Install this state at this point.
    pub install: Option<(u32, PpState)>,
    /// Take a snapshot against the baseline at this point.
    pub checkpoint: Option<u32>,
    /// The baseline, once taken.
    pub taken_baseline: Option<Baseline>,
    /// The checkpoint snapshot, once taken.
    pub taken: Option<PpState>,
    /// The points passed so far (0 to begin with).
    pub point: u32,
}

impl Preprocessor<'_, '_> {
    /// Installs `hooks` (spec/pp.md §13); call before reading the first
    /// token.
    pub fn set_state_hooks(&mut self, hooks: StateHooks) {
        self.state_hooks = Some(Box::new(hooks));
    }

    /// Removes and returns the hooks with what they took.
    pub fn take_state_hooks(&mut self) -> Option<StateHooks> {
        self.state_hooks.take().map(|b| *b)
    }

    /// A state point of the predefines buffer was reached at `at`.
    pub(crate) fn state_point(&mut self, at: Loc) {
        let Some(mut h) = self.state_hooks.take() else { return };
        let k = h.point;
        h.point += 1;
        if k == 0 && h.baseline {
            h.taken_baseline = Some(self.state_baseline());
        }
        if h.install.as_ref().is_some_and(|(p, _)| *p == k)
            && let Some((_, st)) = h.install.take()
        {
            self.install_state(&st, at);
        }
        if h.checkpoint == Some(k)
            && let Some(b) = &h.taken_baseline
        {
            h.taken = Some(self.state_snapshot(b));
        }
        self.state_hooks = Some(h);
    }

    /// True if the top frame is the main file's predefines buffer
    /// itself (not a header unit's copy of it).
    pub(crate) fn in_predefines(&self) -> bool {
        self.frames.len() == 2
            && self.predef_file.is_some()
            && matches!(self.frames.last(), Some(Frame::File(f)) if Some(f.file) == self.predef_file)
    }

    /// The state now, as a baseline for [`Self::state_snapshot`].
    #[must_use]
    pub fn state_baseline(&self) -> Baseline {
        Baseline {
            macros: self.macros.map.clone(),
            pushed: self.pushed.clone(),
            poisoned: self.poisoned.keys().copied().collect(),
            guards: self.guards.keys().copied().collect(),
            once: self.once_files.iter().copied().collect(),
            attr_len: self.attr_macros.len(),
        }
    }

    /// The state now, as the difference from `base`. At the end of the
    /// main file (the main file's frame alone left) its controlling
    /// macro counts as a guard of it, as it would once the file is
    /// included.
    #[must_use]
    pub fn state_snapshot(&self, base: &Baseline) -> PpState {
        let name = |s: Symbol| self.interner.as_str(s);
        let mut macros: Vec<Rc<MacroDef>> = self
            .macros
            .map
            .iter()
            .filter(|(k, d)| base.macros.get(k).is_none_or(|b| !Rc::ptr_eq(b, d)))
            .map(|(_, d)| d.clone())
            .collect();
        macros.sort_by(|a, b| name(a.name).cmp(name(b.name)));
        let mut undefs: Vec<Symbol> =
            base.macros.keys().filter(|k| !self.macros.map.contains_key(k)).copied().collect();
        undefs.sort_by(|a, b| name(*a).cmp(name(*b)));
        let same = |a: &[Option<Rc<MacroDef>>], b: &[Option<Rc<MacroDef>>]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(x, y)| match (x, y) {
                    (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                    (None, None) => true,
                    _ => false,
                })
        };
        let mut pushed: Vec<(Symbol, Vec<Option<Rc<MacroDef>>>)> = self
            .pushed
            .iter()
            .filter(|(k, v)| base.pushed.get(k).is_none_or(|b| !same(b, v)))
            .map(|(k, v)| (*k, v.clone()))
            .chain(
                base.pushed
                    .iter()
                    .filter(|(k, v)| !v.is_empty() && !self.pushed.contains_key(k))
                    .map(|(k, _)| (*k, Vec::new())),
            )
            .collect();
        pushed.sort_by(|a, b| name(a.0).cmp(name(b.0)));
        let mut poisoned: Vec<(Symbol, Span)> = self
            .poisoned
            .iter()
            .filter(|(k, _)| !base.poisoned.contains(k))
            .map(|(k, s)| (*k, *s))
            .collect();
        poisoned.sort_by(|a, b| name(a.0).cmp(name(b.0)));
        let mut guards: Vec<(FileId, Symbol)> = self
            .guards
            .iter()
            .filter(|(f, _)| !base.guards.contains(f))
            .map(|(f, m)| (*f, *m))
            .collect();
        if self.frames.len() == 1
            && let Some(Frame::File(f)) = self.frames.first()
            && Some(f.file) == self.main_file
            && f.conds.is_empty()
            && let Some(m) = f.mi.controlling_macro()
            && !guards.iter().any(|g| g.0 == f.file)
        {
            guards.push((f.file, m));
        }
        guards.sort_by_key(|g| g.0);
        let mut once: Vec<FileId> =
            self.once_files.iter().filter(|f| !base.once.contains(f)).copied().collect();
        once.sort();
        PpState {
            macros,
            undefs,
            pushed,
            poisoned,
            guards,
            once,
            counter: self.counter,
            diag_pushes: self.diag_pushes,
            attr: self.attr_macros.get(base.attr_len..).unwrap_or_default().to_vec(),
            diag: self.diags.pragma_snapshot(),
            sema: SemaPragmas::default(),
        }
    }

    /// Makes `st` this preprocessor's state, as if the header it was
    /// taken after had been read at `at` (spec/pp.md §13).
    pub fn install_state(&mut self, st: &PpState, at: Loc) {
        for d in &st.macros {
            self.macros.map.insert(d.name, d.clone());
        }
        for u in &st.undefs {
            self.macros.map.remove(u);
        }
        for (n, v) in &st.pushed {
            self.pushed.insert(*n, v.clone());
        }
        for &(n, s) in &st.poisoned {
            self.poisoned.insert(n, s);
        }
        let main_dir = self.main_file.map(|m| crate::search::parent_dir(self.sm.file_name(m)));
        for &(f, m) in &st.guards {
            if let Some(h) = self.resolver.suggest_include(self.sm, f, main_dir) {
                self.sm.mark_include_guarded(f, h.into());
            }
            self.guards.insert(f, m);
            self.entered.insert(f);
        }
        for &f in &st.once {
            self.once_files.insert(f);
            self.entered.insert(f);
        }
        self.counter = st.counter;
        self.diag_pushes = self.diag_pushes.saturating_add(st.diag_pushes);
        if !st.attr.is_empty() {
            for a in &st.attr {
                if !a.end.is_valid() {
                    let n = self.interner.intern(&a.name);
                    self.attr_live.insert(n, self.attr_macros.len());
                }
                self.attr_macros.push(a.clone());
            }
            self.attr_macro_gen = self.attr_macro_gen.wrapping_add(1);
        }
        if !st.diag.states.is_empty() {
            self.diags.pragma_install(at, &st.diag);
        }
    }

    /// The round trip of a header unit's exported macros through the
    /// record (spec/modules.md §11.12 "Header units"): encoded with the
    /// unit's file `unit` as the main file, decoded and materialized.
    /// `None` when any step fails (the caller keeps `defs`).
    pub(crate) fn unit_round_trip(
        &self,
        unit: FileId,
        defs: &[Rc<MacroDef>],
    ) -> Option<Vec<Rc<MacroDef>>> {
        let st = PpState { macros: defs.to_vec(), ..PpState::default() };
        let bytes = st.encode(self.sm, self.interner, unit).ok()?;
        let rec = PpRecord::decode(&bytes).ok()?;
        let mut back = rec.materialize(self.sm, self.interner).ok()?;
        // The encoding sorts by name; the export order is by location.
        let pos: HashMap<Symbol, usize> = defs.iter().enumerate().map(|(i, d)| (d.name, i)).collect();
        back.macros.sort_by_key(|d| pos.get(&d.name).copied().unwrap_or(usize::MAX));
        (back.macros.len() == defs.len()).then_some(back.macros)
    }
}

// --- encoding -----------------------------------------------------------

fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_len(out: &mut Vec<u8>, n: usize) {
    put32(out, u32::try_from(n).unwrap_or(u32::MAX));
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_len(out, s.len());
    out.extend_from_slice(s.as_bytes());
}

const DEF_USER: u8 = 0;
const DEF_BUILTIN: u8 = 1;

const F_FUNCTION_LIKE: u8 = 1;
const F_VARIADIC: u8 = 2;
const F_NAMED_VARIADIC: u8 = 4;
const F_USED: u8 = 8;
const F_ALLOW_REDEF: u8 = 16;

const T_IDENT: u8 = 0;
const T_NUMBER: u8 = 1;
const T_CHAR: u8 = 2;
const T_STRING: u8 = 3;
const T_RAW_STRING: u8 = 4;
const T_HEADER_NAME: u8 = 5;
const T_PUNCT: u8 = 6;
const T_OTHER: u8 = 7;
const T_UNKNOWN: u8 = 8;
const T_COMMENT: u8 = 9;

const ENCODINGS: [Encoding; 5] =
    [Encoding::Ordinary, Encoding::Wide, Encoding::Utf8, Encoding::Utf16, Encoding::Utf32];

fn enc_code(e: Encoding) -> u32 {
    ENCODINGS.iter().position(|&x| x == e).unwrap_or(0) as u32
}

const SEVERITIES: [Severity; 5] =
    [Severity::Ignored, Severity::Remark, Severity::Warning, Severity::Error, Severity::Fatal];

/// The tag and payload of a body token's kind.
fn kind_code(k: TokKind, name: &mut impl FnMut(Symbol) -> u32) -> W<(u8, u32)> {
    Ok(match k {
        TokKind::Identifier(s) => (T_IDENT, name(s)),
        TokKind::PpNumber => (T_NUMBER, 0),
        TokKind::CharLit(e) => (T_CHAR, enc_code(e)),
        TokKind::StringLit(e) => (T_STRING, enc_code(e)),
        TokKind::RawStringLit(e) => (T_RAW_STRING, enc_code(e)),
        TokKind::HeaderName => (T_HEADER_NAME, 0),
        TokKind::Punct(p) => {
            (T_PUNCT, Punct::ALL.iter().position(|&q| q == p).ok_or("an unknown punctuator")? as u32)
        }
        TokKind::Other(c) => (T_OTHER, u32::from(c)),
        TokKind::Unknown => (T_UNKNOWN, 0),
        TokKind::Comment => (T_COMMENT, 0),
        TokKind::Eod | TokKind::Eof => return Err("an end-of-line token in a macro body"),
    })
}

/// The writer's state: the location table, the names and the
/// definitions made so far.
struct Enc<'s> {
    sm: &'s SourceManager,
    interner: &'s Interner,
    locs: LocTable,
    names: Vec<&'s str>,
    name_ix: HashMap<Symbol, u32>,
    defs: Vec<u8>,
    ndefs: u32,
    def_ix: HashMap<*const MacroDef, u32>,
}

impl<'s> Enc<'s> {
    fn name(&mut self, s: Symbol) -> u32 {
        if let Some(&i) = self.name_ix.get(&s) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(self.interner.as_str(s));
        self.name_ix.insert(s, i);
        i
    }

    fn loc(&mut self, l: Loc) -> W<u32> {
        self.locs.loc(self.sm, l)
    }

    /// The index of `d` in the definition table, encoding it first.
    fn def(&mut self, d: &Rc<MacroDef>) -> W<u32> {
        if let Some(&i) = self.def_ix.get(&Rc::as_ptr(d)) {
            return Ok(i);
        }
        let mut b = Vec::new();
        if let Some(bi) = d.builtin {
            b.push(DEF_BUILTIN);
            let ix = Builtin::ALL.iter().position(|&(_, x)| x == bi).ok_or("an unknown builtin")?;
            put_len(&mut b, ix);
        } else {
            b.push(DEF_USER);
            put32(&mut b, self.name(d.name));
            let flags = [
                (d.function_like, F_FUNCTION_LIKE),
                (d.variadic, F_VARIADIC),
                (d.named_variadic, F_NAMED_VARIADIC),
                (d.used.get(), F_USED),
                (d.allow_redef.get(), F_ALLOW_REDEF),
            ]
            .iter()
            .fold(0u8, |a, &(on, f)| if on { a | f } else { a });
            b.push(flags);
            put32(&mut b, self.loc(d.name_span.lo)?);
            put32(&mut b, self.loc(d.name_span.hi)?);
            put32(&mut b, self.loc(d.def_start)?);
            put32(&mut b, d.def_len);
            put_len(&mut b, d.params.len());
            for &p in &d.params {
                let n = self.name(p);
                put32(&mut b, n);
            }
            if d.rel.len() != d.body.len() {
                return Err("a macro body without its offsets");
            }
            put_len(&mut b, d.body.len());
            for (t, &(lo, hi)) in d.body.iter().zip(&d.rel) {
                if t.tok.span.lo != d.def_start.with_offset(lo)
                    || t.tok.span.hi != d.def_start.with_offset(hi)
                {
                    return Err("a macro body token outside its definition's range");
                }
                let (tag, payload) = kind_code(t.tok.kind, &mut |s| self.name(s))?;
                b.push(tag);
                put32(&mut b, payload);
                b.push(t.tok.flags.bits());
                b.push(t.pf.bits());
                put32(&mut b, lo);
                put32(&mut b, hi);
            }
        }
        self.defs.extend_from_slice(&b);
        let i = self.ndefs;
        self.ndefs += 1;
        self.def_ix.insert(Rc::as_ptr(d), i);
        Ok(i)
    }
}

impl PpState {
    /// The record's bytes ([`crate::bmi::SECTION_PP`]), with `main` as
    /// file 0 of its file table (the header of a PCH, the file of a
    /// header unit). An error names what cannot be recorded (a location
    /// the file table cannot carry, a file that is not valid UTF-8).
    ///
    /// # Errors
    /// The reason the state cannot be recorded.
    pub fn encode(&self, sm: &SourceManager, interner: &Interner, main: FileId) -> W<Vec<u8>> {
        let mut e = Enc {
            sm,
            interner,
            locs: LocTable::with_predefined(sm, main)?,
            names: Vec::new(),
            name_ix: HashMap::new(),
            defs: Vec::new(),
            ndefs: 0,
            def_ix: HashMap::new(),
        };
        let mut rest = Vec::new();
        put_len(&mut rest, self.macros.len());
        for d in &self.macros {
            let i = e.def(d)?;
            put32(&mut rest, i);
        }
        put_len(&mut rest, self.undefs.len());
        for &u in &self.undefs {
            let n = e.name(u);
            put32(&mut rest, n);
        }
        put_len(&mut rest, self.pushed.len());
        for (n, v) in &self.pushed {
            let n = e.name(*n);
            put32(&mut rest, n);
            put_len(&mut rest, v.len());
            for d in v {
                let i = match d {
                    Some(d) => e.def(d)?,
                    None => NONE,
                };
                put32(&mut rest, i);
            }
        }
        put_len(&mut rest, self.poisoned.len());
        for &(n, s) in &self.poisoned {
            let n = e.name(n);
            put32(&mut rest, n);
            put32(&mut rest, e.loc(s.lo)?);
            put32(&mut rest, e.loc(s.hi)?);
        }
        let files: Vec<&(FileId, Symbol)> =
            self.guards.iter().filter(|(f, _)| !sm.is_virtual(*f)).collect();
        put_len(&mut rest, files.len());
        for &&(f, m) in &files {
            put32(&mut rest, e.locs.file_index(sm, f)?);
            let n = e.name(m);
            put32(&mut rest, n);
        }
        let once: Vec<FileId> = self.once.iter().copied().filter(|&f| !sm.is_virtual(f)).collect();
        put_len(&mut rest, once.len());
        for f in once {
            put32(&mut rest, e.locs.file_index(sm, f)?);
        }
        put32(&mut rest, self.counter);
        put32(&mut rest, self.diag_pushes);
        put_len(&mut rest, self.attr.len());
        for a in &self.attr {
            put_str(&mut rest, &a.name);
            put32(&mut rest, e.loc(a.def)?);
            put32(&mut rest, e.loc(a.end)?);
            put_str(&mut rest, &a.tokens);
        }
        put_len(&mut rest, self.diag.states.len());
        for st in &self.diag.states {
            put_len(&mut rest, st.len());
            for &(id, sev, f) in st {
                put_str(&mut rest, id.name());
                rest.push(SEVERITIES.iter().position(|&s| s == sev).unwrap_or(0) as u8);
                rest.push(f);
            }
        }
        put32(&mut rest, self.diag.cur);
        put_len(&mut rest, self.diag.stack.len());
        for &k in &self.diag.stack {
            put32(&mut rest, k);
        }
        let s = &self.sema;
        put32(&mut rest, u32::from(s.pack));
        put_len(&mut rest, s.pack_stack.len());
        for (label, v) in &s.pack_stack {
            match label {
                Some(l) => {
                    rest.push(1);
                    put_str(&mut rest, l);
                }
                None => rest.push(0),
            }
            put32(&mut rest, u32::from(*v));
        }
        rest.push(u8::from(s.ms_struct));
        put_len(&mut rest, s.visibility.len());
        for v in &s.visibility {
            match v {
                Some((code, at)) => {
                    rest.push(1);
                    put32(&mut rest, *code);
                    put32(&mut rest, e.loc(*at)?);
                }
                None => rest.push(0),
            }
        }
        rest.extend_from_slice(&s.fp.to_le_bytes());
        put_len(&mut rest, s.fp_stack.len());
        for f in &s.fp_stack {
            rest.extend_from_slice(&f.to_le_bytes());
        }
        let (files, slocs) = e.locs.finish();
        let mut out = Vec::with_capacity(rest.len() + e.defs.len() + 4096);
        put32(&mut out, VERSION);
        image_locs::encode(&files, &slocs, &mut out);
        put_len(&mut out, e.names.len());
        for n in &e.names {
            put_str(&mut out, n);
        }
        put32(&mut out, e.ndefs);
        out.extend_from_slice(&e.defs);
        out.extend_from_slice(&rest);
        Ok(out)
    }
}

// --- decoding -----------------------------------------------------------

/// A body token as recorded.
#[derive(Clone, Debug)]
struct RawTok {
    tag: u8,
    payload: u32,
    flags: TokFlags,
    pf: PpFlags,
    lo: u32,
    hi: u32,
}

/// A definition as recorded.
#[derive(Clone, Debug)]
enum RawDef {
    Builtin(usize),
    User {
        name: u32,
        flags: u8,
        name_span: [u32; 2],
        def_start: u32,
        def_len: u32,
        params: Vec<u32>,
        toks: Vec<RawTok>,
    },
}

/// A decoded, checked record, not yet tied to a source manager.
#[derive(Clone, Debug)]
pub struct PpRecord {
    files: Vec<FileRec>,
    slocs: Vec<SlocRec>,
    bases: Vec<u32>,
    names: Vec<String>,
    defs: Vec<RawDef>,
    macros: Vec<u32>,
    undefs: Vec<u32>,
    pushed: Vec<(u32, Vec<u32>)>,
    poisoned: Vec<(u32, [u32; 2])>,
    guards: Vec<(u32, u32)>,
    once: Vec<u32>,
    counter: u32,
    diag_pushes: u32,
    attr: Vec<(String, u32, u32, String)>,
    diag: PragmaSnapshot,
    pack: u16,
    pack_stack: Vec<(Option<String>, u16)>,
    ms_struct: bool,
    visibility: Vec<Option<(u32, u32)>>,
    fp: u64,
    fp_stack: Vec<u64>,
}

fn tok_flags(bits: u8) -> W<TokFlags> {
    const ALL: [TokFlags; 6] = [
        TokFlags::LEADING_SPACE,
        TokFlags::START_OF_LINE,
        TokFlags::NEEDS_CLEANING,
        TokFlags::DIGRAPH,
        TokFlags::HAS_UCN,
        TokFlags::UD_SUFFIX,
    ];
    let mut f = TokFlags::empty();
    let mut left = bits;
    for x in ALL {
        if bits & x.bits() != 0 {
            f.insert(x);
            left &= !x.bits();
        }
    }
    if left != 0 {
        return Err("unknown token flags");
    }
    Ok(f)
}

impl PpRecord {
    /// Decodes and checks the bytes of a [`crate::bmi::SECTION_PP`]
    /// section. Total: every malformed input is an error.
    ///
    /// # Errors
    /// What is malformed.
    pub fn decode(bytes: &[u8]) -> W<PpRecord> {
        let mut r = Reader(bytes);
        if r.u32()? != VERSION {
            return Err("an unknown preprocessor-state record version");
        }
        let (files, slocs, bases) = image_locs::decode(&mut r)?;
        let n = r.count(4)?;
        let mut names = Vec::with_capacity(n);
        for _ in 0..n {
            names.push(r.str()?);
        }
        let name_ok = |i: u32| (i as usize) < names.len();
        let loc_ok = |o: u32| Locs::in_space(&bases, &files, &slocs, o);
        let ndefs = r.count(5)?;
        let mut defs = Vec::with_capacity(ndefs);
        for _ in 0..ndefs {
            let d = match r.u8()? {
                DEF_BUILTIN => {
                    let i = r.u32()? as usize;
                    if i >= Builtin::ALL.len() {
                        return Err("an unknown builtin");
                    }
                    RawDef::Builtin(i)
                }
                DEF_USER => {
                    let name = r.u32()?;
                    let flags = r.u8()?;
                    if flags >= 32 {
                        return Err("unknown macro flags");
                    }
                    let name_span = [r.u32()?, r.u32()?];
                    let def_start = r.u32()?;
                    let def_len = r.u32()?;
                    let np = r.count(4)?;
                    let mut params = Vec::with_capacity(np);
                    for _ in 0..np {
                        let p = r.u32()?;
                        if !name_ok(p) {
                            return Err("a name index out of range");
                        }
                        params.push(p);
                    }
                    let nt = r.count(15)?;
                    let mut toks = Vec::with_capacity(nt);
                    for _ in 0..nt {
                        let tag = r.u8()?;
                        let payload = r.u32()?;
                        let flags = tok_flags(r.u8()?)?;
                        let pf = PpFlags::from_bits(r.u8()?).ok_or("unknown token flags")?;
                        let (lo, hi) = (r.u32()?, r.u32()?);
                        let ok = match tag {
                            T_IDENT => name_ok(payload),
                            T_CHAR | T_STRING | T_RAW_STRING => (payload as usize) < ENCODINGS.len(),
                            T_PUNCT => (payload as usize) < Punct::ALL.len(),
                            T_OTHER => char::from_u32(payload).is_some(),
                            T_NUMBER | T_HEADER_NAME | T_UNKNOWN | T_COMMENT => payload == 0,
                            _ => false,
                        };
                        if !ok || lo > hi || hi > def_len {
                            return Err("a malformed macro body token");
                        }
                        toks.push(RawTok { tag, payload, flags, pf, lo, hi });
                    }
                    if !name_ok(name)
                        || !name_span.iter().all(|&o| loc_ok(o))
                        || def_start == NONE
                        || !loc_ok(def_start)
                        || def_start.checked_add(def_len).is_none_or(|e| !loc_ok(e))
                    {
                        return Err("a malformed macro definition");
                    }
                    RawDef::User { name, flags, name_span, def_start, def_len, params, toks }
                }
                _ => return Err("an unknown macro definition kind"),
            };
            defs.push(d);
        }
        let def_ok = |i: u32| (i as usize) < defs.len();
        let file_ok = |i: u32| (i as usize) < files.len() && !files[i as usize].path.is_empty();
        let n = r.count(4)?;
        let mut macros = Vec::with_capacity(n);
        for _ in 0..n {
            let i = r.u32()?;
            if !def_ok(i) {
                return Err("a definition index out of range");
            }
            macros.push(i);
        }
        let n = r.count(4)?;
        let mut undefs = Vec::with_capacity(n);
        for _ in 0..n {
            let i = r.u32()?;
            if !name_ok(i) {
                return Err("a name index out of range");
            }
            undefs.push(i);
        }
        let n = r.count(8)?;
        let mut pushed = Vec::with_capacity(n);
        for _ in 0..n {
            let name = r.u32()?;
            let m = r.count(4)?;
            let mut v = Vec::with_capacity(m);
            for _ in 0..m {
                let i = r.u32()?;
                if i != NONE && !def_ok(i) {
                    return Err("a definition index out of range");
                }
                v.push(i);
            }
            if !name_ok(name) {
                return Err("a name index out of range");
            }
            pushed.push((name, v));
        }
        let n = r.count(12)?;
        let mut poisoned = Vec::with_capacity(n);
        for _ in 0..n {
            let (name, lo, hi) = (r.u32()?, r.u32()?, r.u32()?);
            if !name_ok(name) || !loc_ok(lo) || !loc_ok(hi) {
                return Err("a malformed poisoned identifier");
            }
            poisoned.push((name, [lo, hi]));
        }
        let n = r.count(8)?;
        let mut guards = Vec::with_capacity(n);
        for _ in 0..n {
            let (f, m) = (r.u32()?, r.u32()?);
            if !file_ok(f) || !name_ok(m) {
                return Err("a malformed include guard");
            }
            guards.push((f, m));
        }
        let n = r.count(4)?;
        let mut once = Vec::with_capacity(n);
        for _ in 0..n {
            let f = r.u32()?;
            if !file_ok(f) {
                return Err("a malformed #pragma once file");
            }
            once.push(f);
        }
        let counter = r.u32()?;
        let diag_pushes = r.u32()?;
        let n = r.count(16)?;
        let mut attr = Vec::with_capacity(n);
        for _ in 0..n {
            let name = r.str()?;
            let (def, end) = (r.u32()?, r.u32()?);
            let tokens = r.str()?;
            if !loc_ok(def) || !loc_ok(end) {
                return Err("a malformed attribute macro");
            }
            attr.push((name, def, end, tokens));
        }
        let n = r.count(4)?;
        let mut diag = PragmaSnapshot::default();
        for _ in 0..n {
            let m = r.count(6)?;
            let mut st = Vec::with_capacity(m);
            for _ in 0..m {
                let id = r.str()?;
                let sev = r.u8()?;
                let f = r.u8()?;
                // An id this kiln does not know is dropped (its warning
                // does not exist here).
                let Some(id) = DiagId::from_name(&id) else { continue };
                let sev = *SEVERITIES.get(sev as usize).ok_or("an unknown severity")?;
                if f >= 8 {
                    return Err("unknown diagnostic mapping flags");
                }
                st.push((id, sev, f));
            }
            diag.states.push(st);
        }
        diag.cur = r.u32()?;
        let m = r.count(4)?;
        for _ in 0..m {
            diag.stack.push(r.u32()?);
        }
        let nstates = diag.states.len() as u32;
        if (nstates > 0 || diag.cur != 0) && diag.cur >= nstates
            || diag.stack.iter().any(|&k| k >= nstates)
        {
            return Err("a diagnostic state index out of range");
        }
        let pack = u16::try_from(r.u32()?).map_err(|_| "a #pragma pack value out of range")?;
        let n = r.count(5)?;
        let mut pack_stack = Vec::with_capacity(n);
        for _ in 0..n {
            let label = match r.u8()? {
                0 => None,
                1 => Some(r.str()?),
                _ => return Err("a bad #pragma pack entry"),
            };
            let v = u16::try_from(r.u32()?).map_err(|_| "a #pragma pack value out of range")?;
            pack_stack.push((label, v));
        }
        let ms_struct = match r.u8()? {
            0 => false,
            1 => true,
            _ => return Err("a bad flag byte"),
        };
        let n = r.count(1)?;
        let mut visibility = Vec::with_capacity(n);
        for _ in 0..n {
            visibility.push(match r.u8()? {
                0 => None,
                1 => {
                    let (code, at) = (r.u32()?, r.u32()?);
                    if code > 2 || !loc_ok(at) {
                        return Err("a bad visibility entry");
                    }
                    Some((code, at))
                }
                _ => return Err("a bad visibility entry"),
            });
        }
        let fp = r.u64()?;
        let n = r.count(8)?;
        let mut fp_stack = Vec::with_capacity(n);
        for _ in 0..n {
            fp_stack.push(r.u64()?);
        }
        if !r.0.is_empty() {
            return Err("trailing bytes in the preprocessor-state record");
        }
        Ok(PpRecord {
            files,
            slocs,
            bases,
            names,
            defs,
            macros,
            undefs,
            pushed,
            poisoned,
            guards,
            once,
            counter,
            diag_pushes,
            attr,
            diag,
            pack,
            pack_stack,
            ms_struct,
            visibility,
            fp,
            fp_stack,
        })
    }

    /// The number of macro definitions and of files the record holds.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        (self.macros.len(), self.files.len())
    }

    /// The record in `sm`: its files as buffers (a file `sm` has loaded
    /// is used when its text is the recorded one; any other is added as
    /// a lazy buffer, read only when a token in it is printed), its
    /// locations recreated, its names interned.
    ///
    /// # Errors
    /// A file that is missing or no longer holds the recorded text, or
    /// location space running out.
    pub fn materialize(&self, sm: &SourceManager, interner: &Interner) -> W<PpState> {
        let mut ids = Vec::with_capacity(self.files.len());
        for f in &self.files {
            let id = match &f.text {
                Some(t) => sm.add_virtual(&f.name, t.clone()).map_err(|_| "no location space")?,
                None => match sm.add_lazy_file(&f.name, Path::new(&f.path), f.len, f.hash, None) {
                    LazyFile::Added(id) => id,
                    LazyFile::Loaded(id) if sm.same_content(id, f.len, f.hash) => id,
                    LazyFile::Loaded(_) | LazyFile::Changed => {
                        return Err("a file changed since the record was written");
                    }
                    LazyFile::Missing => return Err("a file of the record is missing"),
                    LazyFile::Exhausted => return Err("no location space"),
                },
            };
            ids.push(id);
        }
        let locs = Locs::build(sm, &self.files, &self.slocs, &self.bases, &ids)?;
        let syms: Vec<Symbol> = self.names.iter().map(|n| interner.intern(n)).collect();
        let loc = |o: u32| locs.get(o);
        let mut defs: Vec<Rc<MacroDef>> = Vec::with_capacity(self.defs.len());
        for d in &self.defs {
            let def = match d {
                RawDef::Builtin(i) => {
                    let (n, b) = Builtin::ALL[*i];
                    builtin_def(interner.intern(n), b)
                }
                RawDef::User { name, flags, name_span, def_start, def_len, params, toks } => {
                    let start = loc(*def_start);
                    // Every body token lies in the definition's range,
                    // which one location record holds.
                    let end = loc(def_start + def_len);
                    if !start.is_valid() || end != start.with_offset(*def_len) {
                        return Err("a macro definition crossing location records");
                    }
                    let body: Box<[PpTok]> = toks
                        .iter()
                        .map(|t| {
                            let kind = match t.tag {
                                T_IDENT => TokKind::Identifier(syms[t.payload as usize]),
                                T_NUMBER => TokKind::PpNumber,
                                T_CHAR => TokKind::CharLit(ENCODINGS[t.payload as usize]),
                                T_STRING => TokKind::StringLit(ENCODINGS[t.payload as usize]),
                                T_RAW_STRING => {
                                    TokKind::RawStringLit(ENCODINGS[t.payload as usize])
                                }
                                T_HEADER_NAME => TokKind::HeaderName,
                                T_PUNCT => TokKind::Punct(Punct::ALL[t.payload as usize]),
                                T_OTHER => TokKind::Other(char::from_u32(t.payload).unwrap_or('?')),
                                T_UNKNOWN => TokKind::Unknown,
                                _ => TokKind::Comment,
                            };
                            let span = Span::new(start.with_offset(t.lo), start.with_offset(t.hi));
                            PpTok { tok: Token { kind, span, flags: t.flags }, pf: t.pf }
                        })
                        .collect();
                    Rc::new(MacroDef {
                        name: syms[*name as usize],
                        name_span: Span::new(loc(name_span[0]), loc(name_span[1])),
                        params: params.iter().map(|&p| syms[p as usize]).collect(),
                        function_like: flags & F_FUNCTION_LIKE != 0,
                        variadic: flags & F_VARIADIC != 0,
                        named_variadic: flags & F_NAMED_VARIADIC != 0,
                        body,
                        rel: toks.iter().map(|t| (t.lo, t.hi)).collect(),
                        def_start: start,
                        def_len: *def_len,
                        builtin: None,
                        enabled: Cell::new(true),
                        used: Cell::new(flags & F_USED != 0),
                        allow_redef: Cell::new(flags & F_ALLOW_REDEF != 0),
                    })
                }
            };
            defs.push(def);
        }
        let def = |i: u32| defs[i as usize].clone();
        Ok(PpState {
            macros: self.macros.iter().map(|&i| def(i)).collect(),
            undefs: self.undefs.iter().map(|&i| syms[i as usize]).collect(),
            pushed: self
                .pushed
                .iter()
                .map(|(n, v)| {
                    (syms[*n as usize], v.iter().map(|&i| (i != NONE).then(|| def(i))).collect())
                })
                .collect(),
            poisoned: self
                .poisoned
                .iter()
                .map(|(n, s)| (syms[*n as usize], Span::new(loc(s[0]), loc(s[1]))))
                .collect(),
            guards: self.guards.iter().map(|&(f, m)| (ids[f as usize], syms[m as usize])).collect(),
            once: self.once.iter().map(|&f| ids[f as usize]).collect(),
            counter: self.counter,
            diag_pushes: self.diag_pushes,
            attr: self
                .attr
                .iter()
                .map(|(name, def, end, tokens)| AttrMacro {
                    name: name.as_str().into(),
                    def: loc(*def),
                    end: loc(*end),
                    tokens: tokens.as_str().into(),
                })
                .collect(),
            diag: self.diag.clone(),
            sema: SemaPragmas {
                pack: self.pack,
                pack_stack: self.pack_stack.clone(),
                ms_struct: self.ms_struct,
                visibility: self.visibility.iter().map(|v| v.map(|(c, at)| (c, loc(at)))).collect(),
                fp: self.fp,
                fp_stack: self.fp_stack.clone(),
            },
        })
    }
}

/// A builtin macro's definition (clang `RegisterBuiltinMacros`).
pub(crate) fn builtin_def(name: Symbol, b: Builtin) -> Rc<MacroDef> {
    Rc::new(MacroDef {
        name,
        name_span: Span::INVALID,
        params: Box::new([]),
        function_like: false,
        variadic: false,
        named_variadic: false,
        body: Box::new([]),
        rel: Box::new([]),
        def_start: Loc::INVALID,
        def_len: 0,
        builtin: Some(b),
        enabled: Cell::new(true),
        used: Cell::new(false),
        allow_redef: Cell::new(false),
    })
}

// --- comparison ---------------------------------------------------------

impl PpState {
    /// The state as sorted canonical lines, for the self-check that
    /// compares a record with replay (`-fkiln-pch-image=check`, spec/
    /// driver.md): each macro with its parameters, its body's spellings
    /// and flags and its definition's place, the undefs, the
    /// `push_macro` stacks, the poisoned identifiers, the guards and
    /// `#pragma once` files by file name, the counter and the
    /// diagnostic pragma state by content. Sema's pragma state is not
    /// included.
    #[must_use]
    pub fn describe(&self, sm: &SourceManager, interner: &Interner) -> Vec<String> {
        let name = |s: Symbol| interner.as_str(s);
        let at = |l: Loc| -> String {
            match sm.offset_of(l) {
                Some((f, off)) => format!("{}+{off}", sm.file_name(f)),
                None if !l.is_valid() => "-".to_owned(),
                None => "?".to_owned(),
            }
        };
        let def = |d: &MacroDef| -> String {
            if let Some(b) = d.builtin {
                return format!("builtin {:?}", b);
            }
            let mut s = name(d.name).to_owned();
            if d.function_like {
                let ps: Vec<&str> = d.params.iter().map(|&p| name(p)).collect();
                let _ = write!(s, "({})", ps.join(","));
            }
            if d.variadic {
                s.push_str(if d.named_variadic { " named-variadic" } else { " variadic" });
            }
            s.push_str(" =");
            for t in &d.body {
                let text = sm.span_text(t.tok.span).unwrap_or("?");
                let _ = write!(s, " {text}/{}.{}", t.tok.flags.bits(), t.pf.bits());
            }
            let _ = write!(s, " @{}", at(d.name_span.lo));
            s
        };
        let mut v = Vec::new();
        for d in &self.macros {
            v.push(format!("define {}", def(d)));
        }
        for &u in &self.undefs {
            v.push(format!("undef {}", name(u)));
        }
        for (n, stack) in &self.pushed {
            let items: Vec<String> =
                stack.iter().map(|d| d.as_ref().map_or_else(|| "none".to_owned(), |d| def(d))).collect();
            v.push(format!("push {}: [{}]", name(*n), items.join(" | ")));
        }
        for &(n, s) in &self.poisoned {
            v.push(format!("poison {} @{}", name(n), at(s.lo)));
        }
        for &(f, m) in &self.guards {
            if !sm.is_virtual(f) {
                v.push(format!("guard {} {}", sm.file_name(f), name(m)));
            }
        }
        for &f in &self.once {
            if !sm.is_virtual(f) {
                v.push(format!("once {}", sm.file_name(f)));
            }
        }
        v.push(format!("counter {}", self.counter));
        v.push(format!("diag-pushes {}", self.diag_pushes));
        let state = |k: u32| -> String {
            let Some(st) = self.diag.states.get(k as usize) else { return "{}".to_owned() };
            let items: Vec<String> =
                st.iter().map(|(id, sev, f)| format!("{}={sev:?}/{f}", id.name())).collect();
            format!("{{{}}}", items.join(" "))
        };
        v.push(format!("diag {}", state(self.diag.cur)));
        let stack: Vec<String> = self.diag.stack.iter().map(|&k| state(k)).collect();
        v.push(format!("diag-stack [{}]", stack.join(" ")));
        for a in &self.attr {
            v.push(format!("attr {} @{} end @{} {}", a.name, at(a.def), at(a.end), a.tokens));
        }
        v.sort();
        v
    }
}
