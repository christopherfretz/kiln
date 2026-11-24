//! Function-name spans (spec/ast.md §24, WP-3280): for every
//! function-name string kiln makes (`__func__`, `__FUNCTION__`,
//! `__PRETTY_FUNCTION__`, `__builtin_FUNCTION()`, the function name of
//! `std::source_location` and of a contract site), an opt-in side table
//! of the declarations the string prints, each with its byte range in the
//! string and its role. Tooling clients use it to know which bytes of a
//! computed string a rename changes.
//!
//! The printers record through a [`FnSpanSink`] on
//! [`crate::dump::AstNames::spans`]: with a sink they write in-band
//! markers (private-use characters) around every declaration name and
//! every template argument list, which [`FnSpanSink::strip`] removes,
//! computing the ranges, depths and qualifier links. Without a sink no
//! marker is ever written, so the bytes are the printers' own.

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;

use kiln_source::fx::FxHashMap as HashMap;
use kiln_source::{FileId, Loc, SourceManager};

use crate::Ast;
use crate::consts::{PtrBase, Scalar};
use crate::expr::{SrcLocRole, StringMaker};
use crate::ids::{ConstValueId, DeclId, ExprId, StringLitId};
use crate::mem::MemRow;
use crate::names::DeclNameView;

/// A dependent name in a type as the pattern writes it: no declaration.
pub const DEPENDENT: u8 = 1;
/// The bytes are an implementation spelling, not a declared name:
/// `(anonymous namespace)`, `(lambda at F:L:C)`, `(lambda)`,
/// `(anonymous class)`, `(unnamed struct at F:L:C)`, `type-parameter-D-I`.
pub const SYNTHESIZED: u8 = 2;
/// A constructor or destructor `Name` entry whose bytes are its class's
/// name.
pub const CLASS_NAME: u8 = 4;
/// Written right after a `::` whose qualifier ends with the nearest
/// preceding entry at the same depth.
pub const QUALIFIED: u8 = 8;
/// A `Binding` entry of an abbreviated template's invented parameter,
/// naming the function parameter whose name it borrows.
pub const INVENTED: u8 = 16;
/// Reserved (the client review keeps one bit free for a dependent name's
/// resolution in a specialization).
pub const RESERVED_32: u8 = 32;

/// Which part of the string a name is in (spec/ast.md §24.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum FnNameRole {
    /// The function's qualifier, including the template arguments of a
    /// scope class and a scope function's parameter list.
    Scope,
    /// The function's own name, including a conversion function's target
    /// type.
    #[default]
    Name,
    /// The return type, as the pattern writes it.
    Return,
    /// A parameter type, as written.
    Param,
    /// The `P` of `[P = A]`.
    Binding,
    /// The `A` of `[P = A]`.
    Arg,
}

impl FnNameRole {
    /// The dump's spelling.
    pub fn spelling(self) -> &'static str {
        match self {
            FnNameRole::Scope => "scope",
            FnNameRole::Name => "name",
            FnNameRole::Return => "return",
            FnNameRole::Param => "param",
            FnNameRole::Binding => "binding",
            FnNameRole::Arg => "arg",
        }
    }
}

/// One name printed in a function-name string (16 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FnNameSpan {
    /// The first byte of the name in the string.
    pub lo: u32,
    /// One past its last byte.
    pub hi: u32,
    /// The declaration whose name these bytes print (its first
    /// declaration, [`Ast::first`]); `None` only with [`DEPENDENT`].
    pub entity: Option<DeclId>,
    /// Which part of the string the name is in.
    pub role: FnNameRole,
    /// [`DEPENDENT`], [`SYNTHESIZED`], [`CLASS_NAME`], [`QUALIFIED`],
    /// [`INVENTED`].
    pub flags: u8,
    /// How many owners enclose the entry: entries whose template argument
    /// list, parameter list (a scope function) or conversion type holds
    /// this one. 0 = top level.
    pub depth: u16,
}

// -- markers ------------------------------------------------------------

/// Opens an entry; followed by two index digits.
const OPEN: char = '\u{F0000}';
/// Closes the innermost open entry.
const CLOSE: char = '\u{F0001}';
/// Opens an owned group (a template argument list, a scope function's
/// parameter list).
const GOPEN: char = '\u{F0002}';
/// Closes the innermost group.
const GCLOSE: char = '\u{F0003}';
/// The first index digit (base 4096).
const DIGIT0: u32 = 0xF_1000;
/// The bytes of an open marker with its digits.
const OPEN_LEN: usize = 12;
/// The most entries one string records.
const MAX_PENDING: usize = 4096 * 4096;

/// Whether `c` is in the marker range.
fn is_marker(c: char) -> bool {
    ('\u{F0000}'..='\u{F1FFF}').contains(&c)
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    entity: Option<DeclId>,
    role: FnNameRole,
    flags: u8,
}

/// The recorder the printers write markers for (spec/ast.md §24.3).
#[derive(Debug, Default)]
pub struct FnSpanSink {
    pending: RefCell<Vec<Pending>>,
    role: Cell<FnNameRole>,
    overflow: Cell<bool>,
}

impl FnSpanSink {
    /// An empty sink, role [`FnNameRole::Name`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the role of the entries opened from now on; returns the old.
    pub fn set_role(&self, role: FnNameRole) -> FnNameRole {
        self.role.replace(role)
    }

    /// Opens an entry naming `entity` at the end of `out`; returns where,
    /// for [`Self::close`].
    pub fn open(&self, out: &mut String, entity: Option<DeclId>, flags: u8) -> usize {
        let at = out.len();
        let mut p = self.pending.borrow_mut();
        let i = p.len();
        if i >= MAX_PENDING {
            self.overflow.set(true);
            return at;
        }
        p.push(Pending { entity, role: self.role.get(), flags });
        out.push(OPEN);
        for d in [i / 4096, i % 4096] {
            out.push(char::from_u32(DIGIT0 + d as u32).unwrap_or(CLOSE));
        }
        at
    }

    /// Closes the entry opened at `at`: an entry over no bytes is removed.
    pub fn close(&self, out: &mut String, at: usize) {
        if !out.get(at..).is_some_and(|s| s.starts_with(OPEN)) {
            return;
        }
        if out.len() == at + OPEN_LEN {
            out.truncate(at);
            return;
        }
        out.push(CLOSE);
    }

    /// Opens an owned group.
    pub fn group_open(&self, out: &mut String) {
        out.push(GOPEN);
    }

    /// Closes the innermost owned group.
    pub fn group_close(&self, out: &mut String) {
        out.push(GCLOSE);
    }

    /// `s` without its leading markers (for the printers that inspect
    /// their own output).
    pub fn skip_markers(s: &str) -> &str {
        s.trim_start_matches(is_marker)
    }

    /// `s` without its trailing markers.
    pub fn trim_markers_end(s: &str) -> &str {
        s.trim_end_matches(is_marker)
    }

    /// Removes the markers of `marked`: the bytes and their entries,
    /// sorted by `lo` (at equal `lo`, the owner first), with
    /// [`QUALIFIED`] derived. `None` when the markers are malformed or
    /// the sink overflowed.
    pub fn strip(&self, marked: &str) -> Option<(String, Vec<FnNameSpan>)> {
        if self.overflow.get() {
            return None;
        }
        let pending = self.pending.borrow();
        let mut out = String::with_capacity(marked.len());
        let mut stack: Vec<(usize, u32, u16)> = Vec::new();
        let mut groups: u16 = 0;
        let mut spans = Vec::new();
        let mut chars = marked.chars();
        while let Some(c) = chars.next() {
            match c {
                OPEN => {
                    let hi = u32::from(chars.next()?).checked_sub(DIGIT0)? as usize;
                    let lo = u32::from(chars.next()?).checked_sub(DIGIT0)? as usize;
                    if hi >= 4096 || lo >= 4096 {
                        return None;
                    }
                    let idx = hi * 4096 + lo;
                    pending.get(idx)?;
                    let depth = u16::try_from(stack.len()).ok()?.checked_add(groups)?;
                    stack.push((idx, u32::try_from(out.len()).ok()?, depth));
                }
                CLOSE => {
                    let (idx, lo, depth) = stack.pop()?;
                    let p = pending[idx];
                    spans.push(FnNameSpan {
                        lo,
                        hi: u32::try_from(out.len()).ok()?,
                        entity: p.entity,
                        role: p.role,
                        flags: p.flags,
                        depth,
                    });
                }
                GOPEN => groups = groups.checked_add(1)?,
                GCLOSE => groups = groups.checked_sub(1)?,
                c if is_marker(c) => return None,
                c => out.push(c),
            }
        }
        if !stack.is_empty() || groups != 0 {
            return None;
        }
        spans.sort_by_key(|s| (s.lo, s.depth));
        derive_qualified(out.as_bytes(), &mut spans);
        Some((out, spans))
    }
}

/// Sets [`QUALIFIED`] on each entry written right after `::` whose
/// qualifier ends with the nearest preceding entry at its depth (that
/// entry, then its template arguments or a scope function's parameters).
fn derive_qualified(bytes: &[u8], spans: &mut [FnNameSpan]) {
    for i in 0..spans.len() {
        let s = spans[i];
        let mut lo = s.lo as usize;
        // A destructor's class name: `::~C`, the `~` outside the entry.
        if s.flags & CLASS_NAME != 0 && lo > 0 && bytes.get(lo - 1) == Some(&b'~') {
            lo -= 1;
        }
        if lo < 2 || bytes.get(lo - 2..lo) != Some(b"::") {
            continue;
        }
        let Some(prev) = spans[..i].iter().rev().find(|p| p.depth == s.depth) else { continue };
        let ph = prev.hi as usize;
        let adjacent = ph == lo - 2
            || (ph < lo - 2
                && matches!(bytes.get(ph), Some(b'<' | b'('))
                && lo >= 3
                && matches!(bytes.get(lo - 3), Some(b'>' | b')')));
        if adjacent {
            spans[i].flags |= QUALIFIED;
        }
    }
}

// -- the table ------------------------------------------------------------

/// One record: the entries of string `lit` printed for `function`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FnNameRecord {
    /// The string's bytes.
    pub lit: StringLitId,
    /// The function the string was printed for (the producer's
    /// declaration, a specialization when it is one).
    pub function: DeclId,
    start: u32,
    len: u32,
}

/// The table (spec/ast.md §24). Transient in snapshots and module images
/// (stage B carries it).
#[derive(Clone, Debug, Default)]
pub struct FnNames {
    on: bool,
    verify: bool,
    unrecorded_prefix: bool,
    index: HashMap<(StringLitId, DeclId), u32>,
    records: Vec<FnNameRecord>,
    entries: Vec<FnNameSpan>,
    predefined: HashMap<ExprId, DeclId>,
}

impl FnNames {
    /// Turns recording on or off.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// Whether the table records.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Turns the self-check of every insert on (`KILN_FN_NAMES_VERIFY`,
    /// spec/ast.md §24.7); implies recording.
    pub fn set_verify(&mut self, verify: bool) {
        self.verify = verify;
        self.on |= verify;
    }

    /// Whether inserts are self-checked.
    pub fn verifies(&self) -> bool {
        self.verify
    }

    /// Strings were made in a restored prefix (a PCH) whose records the
    /// table does not have (spec/ast.md §24.6).
    pub fn set_unrecorded_prefix(&mut self) {
        self.unrecorded_prefix = true;
    }

    /// Whether [`Self::set_unrecorded_prefix`] was called.
    pub fn unrecorded_prefix(&self) -> bool {
        self.unrecorded_prefix
    }

    /// Whether `(lit, f)` has a record.
    pub fn has(&self, lit: StringLitId, f: DeclId) -> bool {
        self.index.contains_key(&(lit, f))
    }

    /// Adds the record of `lit` printed for `f` (a second insert of the
    /// same key is ignored).
    pub fn insert(&mut self, lit: StringLitId, f: DeclId, spans: &[FnNameSpan]) {
        if !self.on || self.index.contains_key(&(lit, f)) {
            return;
        }
        let (Ok(start), Ok(len), Ok(at)) = (
            u32::try_from(self.entries.len()),
            u32::try_from(spans.len()),
            u32::try_from(self.records.len()),
        ) else {
            return;
        };
        self.entries.extend_from_slice(spans);
        self.records.push(FnNameRecord { lit, function: f, start, len });
        self.index.insert((lit, f), at);
    }

    /// `PredefinedExpr` literal `lit_expr` names function `f`.
    pub fn note_predefined(&mut self, lit_expr: ExprId, f: DeclId) {
        if self.on {
            self.predefined.insert(lit_expr, f);
        }
    }

    /// The function a `PredefinedExpr`'s literal names.
    pub fn predefined_function(&self, lit_expr: ExprId) -> Option<DeclId> {
        self.predefined.get(&lit_expr).copied()
    }

    /// The entries of `lit` printed for `f`.
    pub fn get(&self, lit: StringLitId, f: DeclId) -> Option<&[FnNameSpan]> {
        let r = self.records.get(*self.index.get(&(lit, f))? as usize)?;
        self.entries.get(r.start as usize..(r.start + r.len) as usize)
    }

    /// The entries of record `r`.
    pub fn entries_of(&self, r: &FnNameRecord) -> &[FnNameSpan] {
        self.entries.get(r.start as usize..(r.start + r.len) as usize).unwrap_or(&[])
    }

    /// The number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether there are no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The number of entries of every record.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Every record, in an order that is a function of the source only
    /// (spec/ast.md §24.4): by the function's location, then the bytes,
    /// then the function's key name (spec/types.md §12.1).
    pub fn iter<'a>(&'a self, ast: &'a Ast) -> impl Iterator<Item = &'a FnNameRecord> + 'a {
        let mut order: Vec<&FnNameRecord> = self.records.iter().collect();
        let key = |d: DeclId| {
            let mut s = String::new();
            crate::dump::AstNames { key: true, ..crate::dump::AstNames::new(ast) }
                .write_key_name(d, &mut s);
            s
        };
        order.sort_by(|a, b| {
            let la = ast.decls.get(a.function).loc().to_raw();
            let lb = ast.decls.get(b.function).loc().to_raw();
            la.cmp(&lb)
                .then_with(|| ast.strings.bytes(a.lit).cmp(ast.strings.bytes(b.lit)))
                .then_with(|| key(a.function).cmp(&key(b.function)))
        });
        order.into_iter()
    }

    /// The memory report's row; `None` while off.
    pub fn mem_row(&self) -> Option<MemRow> {
        if !self.on {
            return None;
        }
        let e = size_of::<FnNameSpan>() as u64;
        let r = size_of::<FnNameRecord>() as u64;
        // A hashbrown slot: the key/value plus one control byte.
        let slot = (size_of::<((StringLitId, DeclId), u32)>() + 1) as u64;
        let pslot = (size_of::<(ExprId, DeclId)>() + 1) as u64;
        let live = self.entries.len() as u64 * e
            + self.records.len() as u64 * r
            + self.index.len() as u64 * slot
            + self.predefined.len() as u64 * pslot;
        let reserved = self.entries.capacity() as u64 * e
            + self.records.capacity() as u64 * r
            + self.index.capacity() as u64 * slot
            + self.predefined.capacity() as u64 * pslot;
        Some(MemRow::new("fn_names", self.entries.len() as u64, live, reserved))
    }

    /// The dump (spec/ast.md §24.6): one block per record.
    pub fn dump(&self, ast: &Ast, sm: &SourceManager, main: Option<FileId>) -> String {
        let pos = |loc: Loc| -> String {
            match sm.line_col(sm.spelling_loc(loc)) {
                Some(lc) if Some(lc.file) == main => format!("{}:{}", lc.line, lc.col),
                Some(lc) => {
                    let n = sm.file_name(lc.file);
                    let base = n.rsplit(['/', '\\']).next().unwrap_or(n);
                    format!("{base}:{}:{}", lc.line, lc.col)
                }
                None => "?".to_owned(),
            }
        };
        // An entity's label: its qualified name, a specialization with
        // its canonical arguments, a parameter unqualified, an unnamed
        // entity by its spelling (`ns::(anonymous namespace)`).
        let mut label_names = crate::dump::AstNames::with_sources(ast, sm);
        label_names.closure_as_lambda = true;
        label_names.scoped_spec_args = true;
        let decl = |d: DeclId, out: &mut String| {
            use crate::decl::DeclKind;
            let n = ast.decls.get(d);
            let mut s = String::new();
            if matches!(
                n.kind(),
                DeclKind::ParmVar
                    | DeclKind::TemplateTypeParm
                    | DeclKind::NonTypeTemplateParm
                    | DeclKind::TemplateTemplateParm
            ) {
                label_names.write_name(d, &mut s);
            } else {
                label_names.write_qualified_spec_name(d, &mut s);
                if let Some((_, info)) = ast.spec_info(d) {
                    crate::types::write_arg_list_canonical(
                        &ast.types,
                        &label_names,
                        info.args,
                        &mut s,
                    );
                }
            }
            if s.is_empty() {
                s.push_str("(anonymous)");
            }
            out.push_str(&s);
            out.push('@');
            out.push_str(&pos(n.loc()));
        };
        let mut out = String::new();
        if self.unrecorded_prefix {
            out.push_str("fn-names: strings of the restored prefix have no records\n");
        }
        for r in self.iter(ast) {
            let bytes = ast.strings.bytes(r.lit);
            let text = String::from_utf8_lossy(bytes);
            let _ = write!(out, "fn-name '{text}' of ");
            decl(r.function, &mut out);
            out.push('\n');
            for s in self.entries_of(r) {
                let t = bytes.get(s.lo as usize..s.hi as usize).unwrap_or(&[]);
                let _ = write!(
                    out,
                    "  [{},{}) {} '{}' -> ",
                    s.lo,
                    s.hi,
                    s.role.spelling(),
                    String::from_utf8_lossy(t)
                );
                match s.entity {
                    Some(d) => decl(d, &mut out),
                    None => out.push_str("(dependent)"),
                }
                if s.depth > 0 {
                    let _ = write!(out, " d={}", s.depth);
                }
                for (bit, word) in [
                    (QUALIFIED, "q"),
                    (DEPENDENT, "dep"),
                    (SYNTHESIZED, "synth"),
                    (CLASS_NAME, "class-name"),
                    (INVENTED, "invented"),
                ] {
                    if s.flags & bit != 0 && !(bit == DEPENDENT && s.entity.is_none()) {
                        out.push(' ');
                        out.push_str(word);
                    }
                }
                out.push('\n');
            }
        }
        out
    }
}

// -- views ----------------------------------------------------------------

/// Which value of a source-location use [`Ast::fn_name_at_use`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FnNameAt {
    /// `__builtin_FUNCTION()` (`SourceLocValues::function`).
    Function,
    /// The `__impl` object's `_M_function_name`.
    Location,
}

/// The entries of one string as a client's pointer sees them.
#[derive(Clone, Copy, Debug)]
pub struct FnNameView<'a> {
    /// The function the string was printed for.
    pub function: DeclId,
    /// The string's contents.
    pub lit: StringLitId,
    /// The whole string (no NUL).
    pub bytes: &'a [u8],
    /// Its entries, sorted by `lo`.
    pub spans: &'a [FnNameSpan],
    /// Where the client's pointer points into `bytes`.
    pub offset: u32,
}

/// An entry as [`FnNameView::visible`] yields it: rebased to the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleSpan {
    /// The entry's index in [`FnNameView::spans`].
    pub index: usize,
    /// Its first byte in the view, clipped at 0.
    pub lo: u32,
    /// One past its last byte in the view, clipped at the view's length.
    pub hi: u32,
    /// Clipping cut the entry.
    pub partial: bool,
}

impl FnNameView<'_> {
    /// The bytes of entry `i`.
    pub fn text(&self, i: usize) -> &[u8] {
        self.spans.get(i).and_then(|s| self.bytes.get(s.lo as usize..s.hi as usize)).unwrap_or(&[])
    }

    /// The owner of entry `i`: the nearest preceding entry one level
    /// shallower.
    pub fn owner(&self, i: usize) -> Option<usize> {
        let d = self.spans.get(i)?.depth.checked_sub(1)?;
        (0..i).rev().find(|&j| self.spans[j].depth == d)
    }

    /// The qualifier of entry `i` when it is [`QUALIFIED`]: the nearest
    /// preceding entry at its depth.
    pub fn qualifier(&self, i: usize) -> Option<usize> {
        let s = self.spans.get(i)?;
        if s.flags & QUALIFIED == 0 {
            return None;
        }
        (0..i).rev().find(|&j| self.spans[j].depth == s.depth)
    }

    /// The entries overlapping `[offset, offset + len)` (to the end with
    /// `None`), rebased to `offset` and clipped to the view, `partial`
    /// when clipping cut them (the client review: clip, never drop).
    pub fn visible(&self, len: Option<u32>) -> impl Iterator<Item = VisibleSpan> + '_ {
        let start = self.offset;
        let total = u32::try_from(self.bytes.len()).unwrap_or(u32::MAX);
        let end = len.map_or(total, |l| start.saturating_add(l).min(total));
        self.spans.iter().enumerate().filter_map(move |(index, s)| {
            if s.hi <= start || s.lo >= end {
                return None;
            }
            let lo = s.lo.max(start);
            let hi = s.hi.min(end);
            Some(VisibleSpan {
                index,
                lo: lo - start,
                hi: hi - start,
                partial: lo != s.lo || hi != s.hi,
            })
        })
    }
}

impl Ast {
    /// The entries of the string object `PtrBase::StringLit { lit, origin }`
    /// seen from byte `offset` (spec/ast.md §24.5; the client review's
    /// accessor at the pointer). `None` in the cases §24.5 lists.
    pub fn fn_name_of_string(
        &self,
        lit: StringLitId,
        origin: ExprId,
        offset: u64,
    ) -> Option<FnNameView<'_>> {
        if !self.fn_names.is_on() {
            return None;
        }
        let f = match self.exprs.string_maker(origin) {
            Some(StringMaker::SourceLoc {
                role: SrcLocRole::Function | SrcLocRole::PrettyFunction,
                decl,
            }) => decl,
            Some(_) => return None,
            None => self.fn_names.predefined_function(origin)?,
        };
        let spans = self.fn_names.get(lit, f)?;
        let bytes = self.strings.bytes(lit);
        if offset > bytes.len() as u64 {
            return None;
        }
        Some(FnNameView { function: f, lit, bytes, spans, offset: offset as u32 })
    }

    /// [`Self::fn_name_of_string`] of frozen pointer `v`.
    pub fn fn_name_of_value(&self, v: ConstValueId) -> Option<FnNameView<'_>> {
        match self.consts.scalar(v)? {
            Scalar::Pointer { base: PtrBase::StringLit { lit, origin }, offset, .. } => {
                self.fn_name_of_string(lit, origin, offset)
            }
            _ => None,
        }
    }

    /// The entries of `PredefinedExpr` `e`'s string.
    pub fn fn_name_of_predefined(&self, e: ExprId) -> Option<FnNameView<'_>> {
        if self.exprs.get(e).kind() != crate::expr::ExprKind::PredefinedExpr {
            return None;
        }
        let sl = self.exprs.predefined_name(e)?;
        self.fn_name_of_string(self.exprs.string_lit(sl), sl, 0)
    }

    /// The entries of the function name at source-location use `u` (a
    /// `SourceLocExpr`, `CXXDefaultArgExpr` or `CXXDefaultInitExpr` with
    /// values): `__builtin_FUNCTION()`'s string, or the `__impl` object's
    /// `_M_function_name`.
    pub fn fn_name_at_use(&self, u: ExprId, at: FnNameAt) -> Option<FnNameView<'_>> {
        let v = self.exprs.source_loc_values(u)?;
        match at {
            FnNameAt::Function => self.fn_name_of_value(v.function),
            FnNameAt::Location => {
                let Scalar::Pointer { base: PtrBase::Const(obj), .. } =
                    self.consts.scalar(v.location?)?
                else {
                    return None;
                };
                self.consts.relocs(obj).iter().find_map(|r| match self.string_maker_of(r.value) {
                    Some((StringMaker::SourceLoc { role: SrcLocRole::PrettyFunction, .. }, _)) => {
                        self.fn_name_of_value(r.value)
                    }
                    _ => None,
                })
            }
        }
    }
}

// -- the self-check ---------------------------------------------------------

/// The words the printers write outside every name (spec/ast.md §24.7):
/// keywords, builtin type spellings and fixed printer words.
const CLOSED_WORDS: &[&str] = &[
    "auto",
    "bool",
    "char",
    "char8_t",
    "char16_t",
    "char32_t",
    "class",
    "const",
    "decltype",
    "double",
    "enum",
    "false",
    "float",
    "int",
    "long",
    "noexcept",
    "nullptr",
    "short",
    "signed",
    "sizeof",
    "alignof",
    "static",
    "struct",
    "template",
    "this",
    "throw",
    "true",
    "typename",
    "union",
    "unsigned",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "restrict",
    "__restrict",
    "_Atomic",
    "_Complex",
    "__int128",
    "_BitInt",
    "_Float16",
    "__fp16",
    "__bf16",
    "__float128",
    "__ibm128",
    "__mfp8",
    "__attribute__",
    "__vector_size__",
    "vector_size",
    "ext_vector_type",
    "matrix_type",
    "_Nonnull",
    "_Nullable",
    "_Null_unspecified",
    "_Nullable_result",
    "__unaligned",
    "dependent",
    "type",
    "expr",
    "requires",
    "static_cast",
    "const_cast",
    "reinterpret_cast",
    "dynamic_cast",
    "new",
    "delete",
    "alignas",
    "typeid",
    "co_await",
    "__func__",
    "__FUNCTION__",
    "__PRETTY_FUNCTION__",
    "__builtin_FUNCTION",
    "__builtin_FILE",
    "__builtin_FILE_NAME",
    "__builtin_LINE",
    "__builtin_COLUMN",
    "__builtin_source_location",
    "__null",
    "__real",
    "__imag",
    "__extension__",
    "_Alignof",
    "__alignof",
    "__alignof__",
    "__builtin_offsetof",
    "__arm_streaming",
    "__arm_streaming_compatible",
    "__arm_inout",
    "__arm_in",
    "__arm_out",
    "__arm_preserves",
    "__arm_agnostic",
    "__arm_new",
    "ptrauth",
    "__ptrauth",
    "_Thread_local",
    "thread_local",
];

/// Whether `word` is in the closed list (or a builtin type family:
/// SVE/NEON/RISC-V builtin types, `__SV*`, `__clang_sv*`).
fn closed_word(word: &str) -> bool {
    CLOSED_WORDS.contains(&word)
        || word.starts_with("__SV")
        || word.starts_with("__clang_sv")
        || word.starts_with("__rvv_")
        || word.starts_with("__builtin_")
}

/// Checks record `spans` of `bytes` (spec/ast.md §24.7): bounds, order,
/// depth structure, entities, that a plain entry's bytes are its entity's
/// identifier, and the converse: every identifier outside all entries is
/// in the closed list. `Err` names the first violation.
pub fn verify(ast: &Ast, bytes: &[u8], spans: &[FnNameSpan]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "not UTF-8".to_owned())?;
    let mut covered = vec![false; bytes.len()];
    let mut prev: Option<FnNameSpan> = None;
    for (i, s) in spans.iter().enumerate() {
        let (lo, hi) = (s.lo as usize, s.hi as usize);
        if lo >= hi || hi > bytes.len() || !text.is_char_boundary(lo) || !text.is_char_boundary(hi)
        {
            return Err(format!("entry {i} [{lo},{hi}) out of bounds"));
        }
        if let Some(p) = prev
            && (p.lo, p.depth) > (s.lo, s.depth)
        {
            return Err(format!("entry {i} out of order"));
        }
        if s.depth > 0 && !spans[..i].iter().any(|p| p.depth == s.depth - 1) {
            return Err(format!("entry {i} at depth {} has no owner", s.depth));
        }
        if s.entity.is_none() != (s.flags & DEPENDENT != 0) {
            return Err(format!("entry {i} '{}': an entity iff not dependent", &text[lo..hi]));
        }
        if s.flags & QUALIFIED != 0 && !spans[..i].iter().any(|p| p.depth == s.depth) {
            return Err(format!("entry {i} qualified without a qualifier"));
        }
        if let Some(d) = s.entity
            && s.flags & SYNTHESIZED == 0
        {
            let named = if s.flags & CLASS_NAME != 0 { ast.decls.get(d).ctx() } else { Some(d) };
            if let Some(n) = named
                && let DeclNameView::Identifier(sym) = ast.names.view(ast.decls.get(n).name())
                && ast.symbols().as_str(sym) != &text[lo..hi]
            {
                return Err(format!(
                    "entry {i} '{}' is not its entity's name '{}'",
                    &text[lo..hi],
                    ast.symbols().as_str(sym)
                ));
            }
        }
        covered[lo..hi].fill(true);
        prev = Some(*s);
    }
    converse(text, &covered)
}

/// The converse check: every identifier outside all entries is a closed
/// word; numbers and character or string literals are skipped.
fn converse(text: &str, covered: &[bool]) -> Result<(), String> {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if covered[i] {
            i += 1;
            continue;
        }
        let c = b[i];
        if c == b'\'' || c == b'"' {
            // A character or string literal (a value argument).
            i += 1;
            while i < b.len() && b[i] != c {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            // A pp-number (`1UL`, `0x1p+0`, `1.5e-3f`).
            i += 1;
            while i < b.len()
                && !covered[i]
                && (b[i].is_ascii_alphanumeric()
                    || b[i] == b'_'
                    || b[i] == b'.'
                    || (matches!(b[i], b'+' | b'-')
                        && matches!(b[i - 1], b'e' | b'E' | b'p' | b'P')))
            {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c >= 0x80 {
            let start = i;
            while i < b.len()
                && !covered[i]
                && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$' || b[i] >= 0x80)
            {
                i += 1;
            }
            let word = &text[start..i];
            if !closed_word(word) {
                return Err(format!("'{word}' at byte {start} names no entry"));
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(i: u32) -> Option<DeclId> {
        DeclId::try_new(i)
    }

    #[test]
    fn entry_is_16_bytes() {
        assert_eq!(size_of::<FnNameSpan>(), 16);
    }

    #[test]
    fn markers_round_trip_with_nesting_and_groups() {
        let sink = FnSpanSink::new();
        let mut out = String::new();
        // `ns::C<ns::S>::f`
        let ns = sink.open(&mut out, d(1), 0);
        out.push_str("ns");
        sink.close(&mut out, ns);
        out.push_str("::");
        let class = sink.open(&mut out, d(2), 0);
        out.push('C');
        sink.close(&mut out, class);
        out.push('<');
        sink.group_open(&mut out);
        let ns = sink.open(&mut out, d(1), 0);
        out.push_str("ns");
        sink.close(&mut out, ns);
        out.push_str("::");
        let st = sink.open(&mut out, d(3), 0);
        out.push('S');
        sink.close(&mut out, st);
        sink.group_close(&mut out);
        out.push_str(">::");
        sink.set_role(FnNameRole::Name);
        let name = sink.open(&mut out, d(4), 0);
        out.push('f');
        sink.close(&mut out, name);
        // An empty name leaves nothing.
        let empty = sink.open(&mut out, d(5), 0);
        sink.close(&mut out, empty);
        let (text, spans) = sink.strip(&out).unwrap();
        assert_eq!(text, "ns::C<ns::S>::f");
        let got: Vec<(u32, u32, u16, bool)> =
            spans.iter().map(|s| (s.lo, s.hi, s.depth, s.flags & QUALIFIED != 0)).collect();
        assert_eq!(
            got,
            [
                (0, 2, 0, false),
                (4, 5, 0, true),
                (6, 8, 1, false),
                (10, 11, 1, true),
                (14, 15, 0, true)
            ]
        );
        let view = FnNameView {
            function: d(4).unwrap(),
            lit: StringLitId::try_new(1).unwrap(),
            bytes: text.as_bytes(),
            spans: &spans,
            offset: 0,
        };
        assert_eq!(view.owner(2), Some(1));
        assert_eq!(view.owner(1), None);
        assert_eq!(view.qualifier(4), Some(1));
        assert_eq!(view.qualifier(3), Some(2));
        assert_eq!(view.text(4), b"f");
    }

    #[test]
    fn nested_entry_is_one_deeper() {
        let sink = FnSpanSink::new();
        let mut out = String::new();
        let c = sink.open(&mut out, d(1), 0);
        out.push_str("operator ");
        let s = sink.open(&mut out, d(2), 0);
        out.push('S');
        sink.close(&mut out, s);
        out.push_str(" *");
        sink.close(&mut out, c);
        let (text, spans) = sink.strip(&out).unwrap();
        assert_eq!(text, "operator S *");
        assert_eq!((spans[0].lo, spans[0].hi, spans[0].depth), (0, 12, 0));
        assert_eq!((spans[1].lo, spans[1].hi, spans[1].depth), (9, 10, 1));
    }

    #[test]
    fn malformed_markers_strip_to_none() {
        let sink = FnSpanSink::new();
        let mut out = String::new();
        let _ = sink.open(&mut out, d(1), 0);
        out.push('x');
        assert!(sink.strip(&out).is_none());
        assert!(sink.strip("a\u{F0001}").is_none());
        assert!(sink.strip("a\u{F0003}").is_none());
    }

    #[test]
    fn visible_clips_and_marks_partial() {
        let spans = [
            FnNameSpan { lo: 0, hi: 2, entity: d(1), role: FnNameRole::Scope, flags: 0, depth: 0 },
            FnNameSpan { lo: 4, hi: 8, entity: d(2), role: FnNameRole::Name, flags: 0, depth: 0 },
        ];
        let view = FnNameView {
            function: d(2).unwrap(),
            lit: StringLitId::try_new(1).unwrap(),
            bytes: b"ns::func",
            spans: &spans,
            offset: 5,
        };
        let v: Vec<VisibleSpan> = view.visible(None).collect();
        assert_eq!(v, [VisibleSpan { index: 1, lo: 0, hi: 3, partial: true }]);
        let view = FnNameView { offset: 0, ..view };
        let v: Vec<VisibleSpan> = view.visible(Some(5)).collect();
        assert_eq!(
            v,
            [
                VisibleSpan { index: 0, lo: 0, hi: 2, partial: false },
                VisibleSpan { index: 1, lo: 4, hi: 5, partial: true }
            ]
        );
    }

    #[test]
    fn converse_rejects_unrecorded_identifiers() {
        let text = "void ns::f(const int *) [T = S]";
        let mut covered = vec![false; text.len()];
        for (lo, hi) in [(5, 7), (9, 10), (29, 30)] {
            covered[lo..hi].fill(true);
        }
        let err = converse(text, &covered).unwrap_err();
        assert!(err.contains("'T'"), "{err}");
        covered[25..26].fill(true);
        assert!(converse(text, &covered).is_ok());
        // Numbers and character literals are no identifiers.
        let text = "int f() [N = 1UL, C = 'a', D = 0x1p+0]";
        let mut covered = vec![false; text.len()];
        for (lo, hi) in [(4, 5), (9, 10), (18, 19), (27, 28)] {
            covered[lo..hi].fill(true);
        }
        assert!(converse(text, &covered).is_ok());
    }

    #[test]
    fn off_records_nothing() {
        let mut t = FnNames::default();
        let lit = StringLitId::try_new(1).unwrap();
        t.insert(lit, d(1).unwrap(), &[]);
        t.note_predefined(ExprId::try_new(1).unwrap(), d(1).unwrap());
        assert!(t.is_empty());
        assert!(t.mem_row().is_none());
        assert!(t.predefined_function(ExprId::try_new(1).unwrap()).is_none());
        t.set_on(true);
        t.insert(lit, d(1).unwrap(), &[]);
        assert_eq!(t.len(), 1);
        assert!(t.mem_row().is_some());
    }
}
