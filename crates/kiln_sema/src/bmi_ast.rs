//! Kind-2 module files, part 1: carried constant-evaluation results
//! (spec/modules.md §11.11).
//!
//! A module interface compiled with a module-file output records the
//! successful results of the constant evaluations sema asks for while it
//! reads the unit (`static_assert` conditions, `constexpr` initializers,
//! immediate invocations, template arguments, `if constexpr` conditions,
//! constraints, array bounds, ...). The driver writes them into the
//! module file as the [`kiln_pp::bmi::SECTION_CONSTS`] section. An
//! importer that rebuilds the unit from the file's capsule answers the
//! same evaluations from the section instead of running the Oven again.
//!
//! Identity. An evaluation is keyed by its kind (plus the context flags
//! that change its result) and two source positions, the construct's
//! (`EvalContext::loc`) and the expression's first token, each as the
//! (path, offset) of its expansion and spelling locations, which the
//! capsule replay reproduces exactly (§11.2). A key the writer saw more
//! than once is dropped, as is every evaluation made inside a template
//! instantiation (one position, many results), one that changed the AST
//! or emitted a diagnostic, and one whose value is not self-contained.
//! The importer consults the table only while it reads the unit the file
//! holds, so the same header text read by the importer itself never
//! matches.
//!
//! Values. Part 1 carries values that need no declaration or type
//! identity: integers, enumerators, `bool`, floating-point values, null
//! pointers and aggregates without pointers or unions. The value's type is
//! not stored: it is the canonical, unqualified type of the evaluated
//! expression (the writer checks that it is), so the importer takes it
//! from its own copy of the expression and checks the decoded value
//! against it before interning it.
//!
//! Reading is total: [`decode`] checks every length against the bytes
//! left before allocating and returns a [`SectionError`] for any malformed
//! input; a decoded value that does not fit its expression's type is not
//! used (the evaluation runs). Nothing here panics on file contents.

use kiln_ast::consts::{ByteImage, ByteRange, FloatKind, IntBits, NullKind, Scalar};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{ConstValueId, ExprId, QualType};
use kiln_oven::{EvalContext, EvalFailure, EvalKind};
use kiln_source::fx::FxHashMap;
use kiln_source::{FileId, Loc, SourceManager};

use crate::Sema;

/// A source position as the capsule replay reproduces it: (path index,
/// offset) of the expansion location, then of the spelling location.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct LocKey([u32; 4]);

/// An evaluation's identity (see the module documentation).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct EvalKey {
    /// [`kind_code`].
    kind: u8,
    /// `EvalContext::loc`.
    at: LocKey,
    /// The evaluated expression's first token.
    expr: LocKey,
}

/// A carried value, without its type.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Carried {
    Int { signed: bool, width: u32, words: Box<[u64]> },
    Bool(bool),
    Float { kind: FloatKind, bits: u128 },
    NullPointer,
    Aggregate { bytes: Box<[u8]>, padding: Box<[ByteRange]> },
}

/// A malformed carried-constants section (reported as the module file's
/// corrupt-file fatal error, spec/modules.md §11.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionError(pub String);

/// The kind byte of an evaluation, or `None` for one that is never
/// carried: a consteval block's evaluation may inject declarations.
fn kind_code(ctx: &EvalContext) -> Option<u8> {
    let k = match ctx.kind {
        EvalKind::StaticAssert => 0,
        EvalKind::Init => 1,
        EvalKind::TemplateArg => 2,
        EvalKind::Consteval => 3,
        EvalKind::IfConstexpr => 4,
        EvalKind::Constraint => 5,
        EvalKind::Other => 6,
        EvalKind::ConstevalBlock => return None,
    };
    if ctx.plain {
        return None;
    }
    let flags = u8::from(ctx.enum_range_check) << 4 | u8::from(ctx.object.is_some()) << 5;
    Some(k | flags)
}

/// Why the first evaluation that was not carried was not (the `-v` note,
/// spec/modules.md §11.11 "Fallback").
pub(crate) type Skip = &'static str;

/// The writer's record of the main unit's evaluations.
#[derive(Default)]
pub(crate) struct Recorder {
    paths: Vec<String>,
    path_ix: FxHashMap<FileId, Option<u32>>,
    /// Each key in first-evaluation order; `None` once it is not carried.
    entries: Vec<(EvalKey, Option<Carried>)>,
    index: FxHashMap<EvalKey, usize>,
    first_skip: Option<Skip>,
}

/// The importer's table for one unit.
struct Table {
    unit: u16,
    paths: FxHashMap<String, u32>,
    file_ix: FxHashMap<FileId, Option<u32>>,
    map: FxHashMap<EvalKey, Carried>,
}

/// Sema's kind-2 state.
#[derive(Default)]
pub(crate) struct BmiState {
    recorder: Option<Recorder>,
    tables: Vec<Table>,
    /// Evaluations answered from a table.
    hits: u64,
    /// The main unit's declaration image is wanted
    /// ([`Sema::record_module_image`], spec/modules.md §11.11 "Part 2a").
    pub(crate) record_image: bool,
}

impl std::fmt::Debug for BmiState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BmiState")
            .field("recording", &self.recorder.as_ref().map(|r| r.entries.len()))
            .field("tables", &self.tables.iter().map(|t| (t.unit, t.map.len())).collect::<Vec<_>>())
            .field("hits", &self.hits)
            .finish()
    }
}

/// What the writer hands the driver: the section and what the `-v` note
/// reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarriedSection {
    /// The [`kiln_pp::bmi::SECTION_CONSTS`] bytes; empty when nothing is
    /// carried.
    pub bytes: Vec<u8>,
    /// Results carried.
    pub carried: usize,
    /// Why the first evaluation that is not carried is not.
    pub first_skip: Option<&'static str>,
}

/// The writer's state at the start of a recorded evaluation.
pub(crate) struct Probe {
    key: EvalKey,
    decls: usize,
    exprs: usize,
    stmts: usize,
    diags: (u32, u32),
}

fn loc_key(
    sm: &SourceManager,
    loc: Loc,
    mut file: impl FnMut(FileId) -> Option<u32>,
) -> Option<LocKey> {
    let (ef, eo) = sm.offset_of(sm.expansion_loc(loc))?;
    let (sf, so) = sm.offset_of(sm.spelling_loc(loc))?;
    Some(LocKey([file(ef)?, eo, file(sf)?, so]))
}

/// A file whose positions are not reproduced by a replay: the predefines,
/// scratch buffers and other `<...>` buffers.
pub(crate) fn unstable_file(name: &str) -> bool {
    name.is_empty() || name.starts_with('<')
}

impl Sema<'_, '_> {
    /// Records the main unit's evaluations for a kind-2 module file
    /// (spec/modules.md §11.11); set by the driver when it writes one.
    pub fn record_carried_constants(&mut self) {
        self.bmi.recorder = Some(Recorder::default());
    }

    /// Evaluations answered from a module file's carried constants.
    pub fn carried_hits(&self) -> u64 {
        self.bmi.hits
    }

    /// The recorded section ([`Sema::record_carried_constants`]), once.
    pub fn take_carried_section(&mut self) -> Option<CarriedSection> {
        let rec = self.bmi.recorder.take()?;
        Some(encode(&rec))
    }

    /// Installs the carried constants `bytes` of module file `path` for
    /// the unit being read now (the imported unit just begun, or the main
    /// unit of a `.pcm` input). A malformed section is the module file's
    /// corrupt-file fatal error at `at`.
    pub fn install_carried_constants(&mut self, path: &str, bytes: &[u8], at: Loc) {
        match decode(bytes) {
            Ok((paths, map)) => {
                let paths = paths.into_iter().enumerate().map(|(i, p)| (p, i as u32)).collect();
                let unit = self.module_current_unit();
                self.bmi.tables.push(Table { unit, paths, file_ix: FxHashMap::default(), map });
            }
            Err(SectionError(why)) => {
                kiln_diag::Diag::new(
                    kiln_diag::DiagId::pp_module_file_corrupt,
                    kiln_source::Span::new(at, at),
                )
                .arg(path)
                .arg(why)
                .emit(self.diags);
            }
        }
    }

    fn eval_key(&mut self, e: ExprId, ctx: &EvalContext, table: Option<usize>) -> Option<EvalKey> {
        let kind = kind_code(ctx)?;
        let sm = self.diags.sm();
        let begin = self.ast.begin_loc(e);
        let mut file = |f: FileId| -> Option<u32> {
            if let Some(t) = table {
                let t = &mut self.bmi.tables[t];
                *t.file_ix.entry(f).or_insert_with(|| {
                    let name = sm.file_name(f);
                    if unstable_file(name) { None } else { t.paths.get(name).copied() }
                })
            } else {
                let r = self.bmi.recorder.as_mut()?;
                *r.path_ix.entry(f).or_insert_with(|| {
                    let name = sm.file_name(f);
                    if unstable_file(name) {
                        return None;
                    }
                    r.paths.push(name.to_owned());
                    Some(r.paths.len() as u32 - 1)
                })
            }
        };
        let at = loc_key(sm, ctx.loc, &mut file)?;
        let expr = loc_key(sm, begin, &mut file)?;
        Some(EvalKey { kind, at, expr })
    }

    /// The carried result of evaluating `e` in `ctx`, if the unit being
    /// read came from a kind-2 module file that holds one which fits.
    pub(crate) fn carried_lookup(&mut self, e: ExprId, ctx: &EvalContext) -> Option<ConstValueId> {
        if self.bmi.tables.is_empty() || !self.inst.stack.is_empty() {
            return None;
        }
        let unit = self.module_current_unit();
        let t = self.bmi.tables.iter().position(|t| t.unit == unit)?;
        let key = self.eval_key(e, ctx, Some(t))?;
        let v = self.bmi.tables[t].map.get(&key)?.clone();
        let id = self.intern_carried(e, &v)?;
        self.bmi.hits += 1;
        Some(id)
    }

    /// The canonical, unqualified type of `e`.
    fn carried_type(&mut self, e: ExprId) -> QualType {
        let q = self.ast.exprs.get(e).ty();
        let c = self.ast.types.canonical(q);
        self.ast.types.unqualified(c)
    }

    /// Interns `v` as a value of `e`'s type, or `None` when it does not
    /// fit that type.
    fn intern_carried(&mut self, e: ExprId, v: &Carried) -> Option<ConstValueId> {
        let ty = self.carried_type(e);
        let builtin = match self.ast.types.canonical_view(ty) {
            TypeView::Builtin(b) => Some(b),
            _ => None,
        };
        let value = match v {
            Carried::Int { signed, width, words } => {
                if builtin == Some(BuiltinType::Bool)
                    || crate::conv::int_width(self, ty) != Some(*width)
                    || crate::conv::int_signed(self, ty) != *signed
                {
                    return None;
                }
                let bits = IntBits::from_words(*width, words)?;
                Scalar::Int { ty, signed: *signed, bits }
            }
            Carried::Bool(value) => {
                if builtin != Some(BuiltinType::Bool) {
                    return None;
                }
                Scalar::Bool { ty, value: *value }
            }
            Carried::Float { kind, bits } => {
                if builtin.and_then(|b| crate::conv::narrow::float_kind(self, b)) != Some(*kind) {
                    return None;
                }
                Scalar::Float { ty, kind: *kind, bits: *bits }
            }
            Carried::NullPointer => {
                if builtin != Some(BuiltinType::NullPtr) && !self.ast.types.is_pointer(ty) {
                    return None;
                }
                Scalar::Null { ty, kind: NullKind::Pointer }
            }
            Carried::Aggregate { bytes, padding } => {
                let (size, _) = crate::target::size_align(&self.ast, ty, self.abi())?;
                if size != bytes.len() as u64 || builtin.is_some() || self.ast.types.is_pointer(ty)
                {
                    return None;
                }
                let img = ByteImage { ty, bytes, relocs: &[], padding, unions: &[] };
                return self.ast.consts.push_aggregate(img).ok();
            }
        };
        self.ast.consts.push_scalar(value).ok()
    }

    /// The writer's state before evaluating `e` in `ctx`, when the main
    /// unit's evaluations are recorded and this one is eligible.
    pub(crate) fn carried_probe(&mut self, e: ExprId, ctx: &EvalContext) -> Option<Probe> {
        self.bmi.recorder.as_ref()?;
        if self.module_current_unit() != 0 {
            return None;
        }
        if !self.inst.stack.is_empty() {
            self.carried_skip("an evaluation inside a template instantiation");
            return None;
        }
        let Some(key) = self.eval_key(e, ctx, None) else {
            self.carried_skip(if kind_code(ctx).is_none() {
                "a consteval block"
            } else {
                "an evaluation at a position a replay does not reproduce"
            });
            return None;
        };
        Some(Probe {
            key,
            decls: self.ast.decls.len(),
            exprs: self.ast.exprs.len(),
            stmts: self.ast.stmts.len(),
            diags: (self.diags.error_count(), self.diags.warning_count()),
        })
    }

    fn carried_skip(&mut self, why: Skip) {
        if let Some(r) = &mut self.bmi.recorder {
            r.first_skip.get_or_insert(why);
        }
    }

    /// Records the result `r` of the evaluation `p` describes.
    pub(crate) fn carried_record(
        &mut self,
        p: &Probe,
        e: ExprId,
        r: &Result<ConstValueId, EvalFailure>,
    ) {
        let carried = match r {
            Ok(v) => {
                if self.ast.decls.len() != p.decls
                    || self.ast.exprs.len() != p.exprs
                    || self.ast.stmts.len() != p.stmts
                {
                    Err("an evaluation that changed the AST")
                } else if (self.diags.error_count(), self.diags.warning_count()) != p.diags {
                    Err("an evaluation that emitted a diagnostic")
                } else if self.ast.consts.ty(*v) != self.carried_type(e) {
                    Err("a value whose type is not its expression's")
                } else {
                    self.carried_value(*v)
                }
            }
            Err(_) => Err("an evaluation that failed"),
        };
        let Some(rec) = &mut self.bmi.recorder else { return };
        let carried = match carried {
            Ok(c) => Some(c),
            Err(why) => {
                rec.first_skip.get_or_insert(why);
                None
            }
        };
        if let Some(&i) = rec.index.get(&p.key) {
            if rec.entries[i].1.take().is_some() {
                rec.first_skip.get_or_insert("two evaluations at one position");
            }
            return;
        }
        rec.index.insert(p.key, rec.entries.len());
        rec.entries.push((p.key, carried));
    }

    /// `v` as a carried value, or why it cannot be one.
    fn carried_value(&self, v: ConstValueId) -> Result<Carried, Skip> {
        let store = &self.ast.consts;
        if let Some(s) = store.scalar(v) {
            return match s {
                Scalar::Int { signed, bits, .. } => {
                    Ok(Carried::Int { signed, width: bits.width(), words: bits.words().into() })
                }
                Scalar::Bool { value, .. } => Ok(Carried::Bool(value)),
                Scalar::Float { kind, bits, .. } => Ok(Carried::Float { kind, bits }),
                Scalar::Null { kind: NullKind::Pointer, .. } => Ok(Carried::NullPointer),
                Scalar::Null { .. } | Scalar::MemberPtr { .. } => Err("a pointer-to-member value"),
                Scalar::Pointer { .. } => Err("a pointer or reference value"),
                Scalar::Reflection { .. } => Err("a reflection value"),
            };
        }
        let Some(img) = store.aggregate(v) else { return Err("an unknown value") };
        if !img.relocs.is_empty() {
            return Err("an aggregate holding a pointer");
        }
        if !img.unions.is_empty() {
            return Err("an aggregate holding a union");
        }
        Ok(Carried::Aggregate { bytes: img.bytes.into(), padding: img.padding.into() })
    }
}

// --- encoding ---------------------------------------------------------

const TAG_INT: u8 = 0;
const TAG_BOOL: u8 = 1;
const TAG_FLOAT: u8 = 2;
const TAG_NULL: u8 = 3;
const TAG_AGGREGATE: u8 = 4;

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_u32(out, u32::try_from(b.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(b);
}

/// The section bytes of `rec`: the path table, then each carried entry in
/// first-evaluation order (deterministic: no hash-map iteration).
fn encode(rec: &Recorder) -> CarriedSection {
    let kept: Vec<(&EvalKey, &Carried)> =
        rec.entries.iter().filter_map(|(k, v)| v.as_ref().map(|v| (k, v))).collect();
    if kept.is_empty() {
        return CarriedSection { bytes: Vec::new(), carried: 0, first_skip: rec.first_skip };
    }
    let mut out = Vec::new();
    put_u32(&mut out, rec.paths.len() as u32);
    for p in &rec.paths {
        put_bytes(&mut out, p.as_bytes());
    }
    put_u32(&mut out, kept.len() as u32);
    for (k, v) in &kept {
        out.push(k.kind);
        for w in k.at.0.iter().chain(&k.expr.0) {
            put_u32(&mut out, *w);
        }
        match v {
            Carried::Int { signed, width, words } => {
                out.push(TAG_INT);
                out.push(u8::from(*signed));
                put_u32(&mut out, *width);
                for w in words {
                    out.extend_from_slice(&w.to_le_bytes());
                }
            }
            Carried::Bool(b) => {
                out.push(TAG_BOOL);
                out.push(u8::from(*b));
            }
            Carried::Float { kind, bits } => {
                out.push(TAG_FLOAT);
                out.push(*kind as u8);
                out.extend_from_slice(&bits.to_le_bytes());
            }
            Carried::NullPointer => out.push(TAG_NULL),
            Carried::Aggregate { bytes, padding } => {
                out.push(TAG_AGGREGATE);
                put_bytes(&mut out, bytes);
                put_u32(&mut out, padding.len() as u32);
                for r in padding {
                    put_u32(&mut out, r.start);
                    put_u32(&mut out, r.len);
                }
            }
        }
    }
    CarriedSection { bytes: out, carried: kept.len(), first_skip: rec.first_skip }
}

/// A checked reader over a section's bytes.
struct Rd<'a>(&'a [u8]);

type Res<T> = Result<T, SectionError>;

fn bad<T>(why: &str) -> Res<T> {
    Err(SectionError(format!("carried constants: {why}")))
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        if n > self.0.len() {
            return bad("truncated");
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Res<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Res<u64> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(a))
    }
    fn u128(&mut self) -> Res<u128> {
        let mut a = [0u8; 16];
        a.copy_from_slice(self.take(16)?);
        Ok(u128::from_le_bytes(a))
    }
    fn flag(&mut self) -> Res<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => bad("bad flag byte"),
        }
    }
    /// A count whose elements take at least `min` bytes each, checked
    /// against the bytes left before anything is sized by it.
    fn count(&mut self, min: usize) -> Res<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min.max(1)) > self.0.len() {
            return bad("length exceeds the section");
        }
        Ok(n)
    }
    fn bytes(&mut self) -> Res<&'a [u8]> {
        let n = self.count(1)?;
        self.take(n)
    }
}

/// The smallest encoded entry: kind, two keys, a tag.
const MIN_ENTRY: usize = 1 + 32 + 1;

type Decoded = (Vec<String>, FxHashMap<EvalKey, Carried>);

/// Decodes a carried-constants section. Total: any malformed input is a
/// [`SectionError`].
fn decode(bytes: &[u8]) -> Res<Decoded> {
    let mut r = Rd(bytes);
    let npaths = r.count(4)?;
    let mut paths = Vec::with_capacity(npaths);
    for _ in 0..npaths {
        match std::str::from_utf8(r.bytes()?) {
            Ok(s) => paths.push(s.to_owned()),
            Err(_) => return bad("invalid UTF-8 in a path"),
        }
    }
    let n = r.count(MIN_ENTRY)?;
    if n == 0 {
        return bad("no entries");
    }
    let mut map = FxHashMap::default();
    map.reserve(n);
    for _ in 0..n {
        let kind = r.u8()?;
        if kind & 0x0f > 6 || kind & 0xc0 != 0 {
            return bad("unknown evaluation kind");
        }
        let mut w = [0u32; 8];
        for x in &mut w {
            *x = r.u32()?;
        }
        if [w[0], w[2], w[4], w[6]].iter().any(|&f| f as usize >= paths.len()) {
            return bad("path index out of range");
        }
        let key = EvalKey {
            kind,
            at: LocKey([w[0], w[1], w[2], w[3]]),
            expr: LocKey([w[4], w[5], w[6], w[7]]),
        };
        let v = match r.u8()? {
            TAG_INT => {
                let signed = r.flag()?;
                let width = r.u32()?;
                if width == 0 || width > IntBits::MAX_WIDTH {
                    return bad("bad integer width");
                }
                let nw = IntBits::words_for(width);
                if nw.saturating_mul(8) > r.0.len() {
                    return bad("truncated");
                }
                let mut words = Vec::with_capacity(nw);
                for _ in 0..nw {
                    words.push(r.u64()?);
                }
                if IntBits::from_words(width, &words).is_none() {
                    return bad("integer bits beyond its width");
                }
                Carried::Int { signed, width, words: words.into() }
            }
            TAG_BOOL => Carried::Bool(r.flag()?),
            TAG_FLOAT => {
                let Some(&kind) = FloatKind::ALL.get(usize::from(r.u8()?)) else {
                    return bad("unknown float kind");
                };
                let bits = r.u128()?;
                if kind.bits() < 128 && bits >> kind.bits() != 0 {
                    return bad("float bits beyond its format");
                }
                Carried::Float { kind, bits }
            }
            TAG_NULL => Carried::NullPointer,
            TAG_AGGREGATE => {
                let bytes: Box<[u8]> = r.bytes()?.into();
                let np = r.count(8)?;
                let mut padding = Vec::with_capacity(np);
                for _ in 0..np {
                    let start = r.u32()?;
                    let len = r.u32()?;
                    let rg = ByteRange { start, len };
                    let prev_end = padding.last().map_or(0, |p: &ByteRange| p.end());
                    if len == 0
                        || rg.end() > bytes.len() as u64
                        || (!padding.is_empty() && u64::from(start) <= prev_end)
                    {
                        return bad("bad padding range");
                    }
                    padding.push(rg);
                }
                Carried::Aggregate { bytes, padding: padding.into() }
            }
            _ => return bad("unknown value tag"),
        };
        if map.insert(key, v).is_some() {
            return bad("duplicate entry");
        }
    }
    if !r.0.is_empty() {
        return bad("trailing bytes");
    }
    Ok((paths, map))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Recorder {
        let key =
            |n: u32| EvalKey { kind: 1, at: LocKey([0, n, 0, n]), expr: LocKey([1, n, 0, 7]) };
        let mut r = Recorder { paths: vec!["m.cppm".into(), "h.h".into()], ..Recorder::default() };
        let vals = [
            Carried::Int { signed: true, width: 32, words: vec![0xffff_fffe].into() },
            Carried::Int { signed: false, width: 128, words: vec![1, 2].into() },
            Carried::Bool(true),
            Carried::Float { kind: FloatKind::Double, bits: 0x4009_21fb_5444_2d18 },
            Carried::NullPointer,
            Carried::Aggregate {
                bytes: vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0].into(),
                padding: vec![ByteRange { start: 9, len: 4 }].into(),
            },
        ];
        for (i, v) in vals.into_iter().enumerate() {
            r.entries.push((key(i as u32), Some(v)));
        }
        r.entries.push((key(99), None));
        r
    }

    /// Every value kind round-trips, the encoding is deterministic, and
    /// entries that are not carried are not written.
    #[test]
    fn round_trip() {
        let rec = sample();
        let s = encode(&rec);
        assert_eq!(s.carried, 6);
        assert_eq!(encode(&rec).bytes, s.bytes, "deterministic");
        let (paths, map) = decode(&s.bytes).unwrap();
        assert_eq!(paths, rec.paths);
        assert_eq!(map.len(), 6);
        for (k, v) in &rec.entries {
            assert_eq!(map.get(k), v.as_ref());
        }
    }

    #[test]
    fn empty_is_not_written() {
        let s = encode(&Recorder::default());
        assert!(s.bytes.is_empty());
        assert!(decode(&s.bytes).is_err());
    }

    /// Every truncation, every single-byte change and every length field
    /// set to large values decodes to a value or a `SectionError`, never a
    /// panic; truncations are always errors.
    #[test]
    fn fuzz_truncate_flip_lengths() {
        let b = encode(&sample()).bytes;
        for n in 0..b.len() {
            assert!(decode(&b[..n]).is_err(), "truncated to {n}");
        }
        for i in 0..b.len() {
            for x in [0x01u8, 0x80, 0xff, 0x7f, 0x10] {
                let mut m = b.clone();
                m[i] ^= x;
                let _ = decode(&m);
            }
        }
        for i in 0..b.len().saturating_sub(4) {
            for v in [u32::MAX, u32::MAX / 2, 0x0100_0000, 0] {
                let mut m = b.clone();
                m[i..i + 4].copy_from_slice(&v.to_le_bytes());
                let _ = decode(&m);
            }
        }
        // A deterministic pseudo-random walk of multi-byte damage.
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..4000 {
            let mut m = b.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = (seed % m.len() as u64) as usize;
                m[i] = (seed >> 32) as u8;
            }
            let _ = decode(&m);
        }
    }

    #[test]
    fn rejects_malformed() {
        let b = encode(&sample()).bytes;
        let mut trailing = b.clone();
        trailing.push(0);
        assert_eq!(decode(&trailing), bad("trailing bytes"));
        // The first entry's `at` path index (after the path table).
        let paths_len = 4 + (4 + 6) + (4 + 3);
        let mut m = b.clone();
        m[paths_len + 4 + 1..paths_len + 4 + 5].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(decode(&m), bad("path index out of range"));
    }
}
