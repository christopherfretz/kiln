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
//! than once with different results is dropped, as is one that changed
//! the AST or emitted a diagnostic, and one whose value is not
//! self-contained. The importer consults the table only while it reads
//! the unit the file holds, so the same header text read by the importer
//! itself never matches.
//!
//! Instantiations (part 2, WP-1046). One position inside a template
//! pattern is evaluated once per specialization, so an evaluation made
//! inside an instantiation is keyed by the instantiation as well: the
//! innermost instantiation frame's entity written as a structural byte
//! string ([`inst`]: the template and its canonical arguments, or the
//! enclosing specialization and the pattern member), stored once in the
//! section and named by index from each entry. An evaluation inside a
//! substitution that is not an instantiation (deduction, constraint
//! checks, default template arguments), or inside an instantiation whose
//! key cannot be formed, is not carried.
//!
//! Values. Part 1 carries values that need no declaration or type
//! identity: integers, enumerators, `bool`, floating-point values, null
//! pointers and aggregates without pointers or unions. WP-1047 adds
//! pointers, references, pointers to members, reflections and aggregates
//! holding them, written by the entities they name in the instantiation
//! keys' entity encoding ([`inst`]); the importer resolves each entity to
//! its own declaration and checks that the declaration's key is the
//! written one (`value.rs`). The value's type is not stored: it is the
//! canonical, unqualified type of the evaluated expression (the writer
//! checks that it is), so the importer takes it from its own copy of the
//! expression and checks the decoded value against it before interning
//! it.
//!
//! Reading is total: [`decode`] checks every length against the bytes
//! left before allocating and returns a [`SectionError`] for any malformed
//! input; a decoded value that does not fit its expression's type is not
//! used (the evaluation runs). Nothing here panics on file contents.

mod inst;
mod value;

use kiln_ast::consts::{ByteImage, ByteRange, ConstStore, FloatKind, IntBits, NullKind, Scalar};
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_oven::{EvalContext, EvalFailure, EvalKind};
use kiln_source::fx::FxHashMap;
use kiln_source::{FileId, Loc, SourceManager};

use crate::Sema;
use crate::templ::stack::FrameKind;

/// A source position as the capsule replay reproduces it: (path index,
/// offset) of the expansion location, then of the spelling location.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct LocKey([u32; 4]);

/// An evaluation's identity (see the module documentation).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct EvalKey {
    /// 0 outside template instantiations, else 1 + the index of the
    /// instantiation's key ([`inst`]) in the section.
    inst: u32,
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
    Int {
        signed: bool,
        width: u32,
        words: Box<[u64]>,
    },
    Bool(bool),
    Float {
        kind: FloatKind,
        bits: u128,
    },
    NullPointer,
    Aggregate {
        bytes: Box<[u8]>,
        padding: Box<[ByteRange]>,
    },
    /// Any other value: its whole encoding, tag first (WP-1047; decoded
    /// against the importer's declarations when it is used).
    Wire(Box<[u8]>),
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
    /// Instantiation keys, by [`EvalKey::inst`] - 1.
    insts: Vec<Box<[u8]>>,
    inst_index: FxHashMap<Box<[u8]>, u32>,
    /// The [`EvalKey::inst`] of each instantiated entity seen, `None`
    /// when its key cannot be formed.
    ent_ix: FxHashMap<FrameSig, Option<u32>>,
    first_skip: Option<Skip>,
}

/// The importer's table for one unit.
struct Table {
    unit: u16,
    paths: FxHashMap<String, u32>,
    file_ix: FxHashMap<FileId, Option<u32>>,
    /// Instantiation key -> [`EvalKey::inst`].
    insts: FxHashMap<Box<[u8]>, u32>,
    /// The [`EvalKey::inst`] of each instantiated entity seen, `None`
    /// when the table carries nothing for it.
    ent_ix: FxHashMap<FrameSig, Option<u32>>,
    map: FxHashMap<EvalKey, Carried>,
    /// The entities [`Carried::Wire`] values name, resolved.
    ix: value::Index,
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
            .field("record_image", &self.record_image)
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
    /// Of those, results of evaluations inside template instantiations.
    pub inst_carried: usize,
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

/// The section path index of file `f` (the table's when `table` is set,
/// else the recorder's, which adds it), or `None` for a file whose
/// positions a replay does not reproduce.
fn file_index(
    bmi: &mut BmiState,
    table: Option<usize>,
    sm: &SourceManager,
    f: FileId,
) -> Option<u32> {
    if let Some(t) = table {
        let t = &mut bmi.tables[t];
        *t.file_ix.entry(f).or_insert_with(|| {
            let name = sm.file_name(f);
            if unstable_file(name) { None } else { t.paths.get(name).copied() }
        })
    } else {
        let r = bmi.recorder.as_mut()?;
        *r.path_ix.entry(f).or_insert_with(|| {
            let name = sm.file_name(f);
            if unstable_file(name) {
                return None;
            }
            r.paths.push(name.to_owned());
            Some(r.paths.len() as u32 - 1)
        })
    }
}

/// How evaluations inside an instantiation frame are keyed.
#[derive(Clone, Copy)]
enum Keying {
    /// By the frame's entity alone (the frames that instantiate a
    /// definition, initializer or default argument of one
    /// specialization).
    Entity,
    /// By the frame's template, entity and arguments, with this frame
    /// code (the substitutions whose frame records all their arguments).
    Subst(u8),
    /// Not at all (alias templates, default-argument checks, partial
    /// specialization matching and the rest).
    None,
}

/// How evaluations inside a frame of kind `k` are keyed.
fn frame_keying(k: FrameKind) -> Keying {
    use FrameKind as K;
    match k {
        K::ClassInstantiation
        | K::MemberClass
        | K::MemberFunction
        | K::FunctionSpec
        | K::StaticDataMember
        | K::VarTemplateInstantiation
        | K::VarTemplateInit
        | K::Enum
        | K::Nsdmi
        | K::DefaultFunctionArg
        | K::ExceptionSpec
        | K::LambdaSubstitution => Keying::Entity,
        K::ConceptCheck => Keying::Subst(0),
        K::ConstraintsCheck => Keying::Subst(1),
        K::DeducedArgSubstitution => Keying::Subst(2),
        K::ExplicitArgSubstitution => Keying::Subst(3),
        K::PriorArgSubstitution => Keying::Subst(4),
        K::DefaultTemplateArg => Keying::Subst(5),
        _ => Keying::None,
    }
}

/// What identifies the innermost frame's key: its keying, entity,
/// template and arguments.
type FrameSig = (Option<u8>, DeclId, DeclId, Option<ArgListId>);

const SKIP_SUBST: Skip = "an evaluation in a substitution that is not an instantiation";
const SKIP_INST_KEY: Skip = "an instantiation whose arguments have no key";

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
            Ok(Decoded { paths, insts, map }) => {
                let paths = paths.into_iter().enumerate().map(|(i, p)| (p, i as u32)).collect();
                let unit = self.module_current_unit();
                self.bmi.tables.push(Table {
                    unit,
                    paths,
                    file_ix: FxHashMap::default(),
                    insts,
                    ent_ix: FxHashMap::default(),
                    map,
                    ix: value::Index::default(),
                });
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

    fn eval_key(
        &mut self,
        e: ExprId,
        ctx: &EvalContext,
        table: Option<usize>,
        inst: u32,
    ) -> Option<EvalKey> {
        let kind = kind_code(ctx)?;
        let sm = self.diags.sm();
        let begin = self.ast.begin_loc(e);
        let bmi = &mut self.bmi;
        let mut file = |f: FileId| file_index(bmi, table, sm, f);
        let at = loc_key(sm, ctx.loc, &mut file)?;
        let expr = loc_key(sm, begin, &mut file)?;
        Some(EvalKey { inst, kind, at, expr })
    }

    /// The innermost frame's signature, `Ok(None)` outside any frame, or
    /// `Err` for a frame whose evaluations are not keyed.
    fn frame_sig(&self) -> Result<Option<FrameSig>, Skip> {
        let Some(f) = self.inst.stack.frames.last() else { return Ok(None) };
        match frame_keying(f.kind) {
            Keying::Entity => Ok(Some((None, f.entity, f.entity, None))),
            Keying::Subst(code) if f.args.is_some() => {
                Ok(Some((Some(code), f.entity, f.template, f.args)))
            }
            Keying::Subst(_) | Keying::None => Err(SKIP_SUBST),
        }
    }

    /// The key bytes of frame `sig`, when they can be formed.
    fn frame_key(&mut self, table: Option<usize>, sig: FrameSig) -> Option<Box<[u8]>> {
        let sm = self.diags.sm();
        let bmi = &mut self.bmi;
        let file = |f| file_index(bmi, table, sm, f);
        match sig {
            (None, entity, ..) => inst::entity_key(&self.ast, sm, file, entity),
            (Some(code), entity, template, Some(args)) => {
                inst::subst_key(&self.ast, sm, file, code, template, entity, args)
            }
            _ => None,
        }
    }

    /// The writer's [`EvalKey::inst`] for an evaluation made now, or why
    /// it has none.
    fn record_inst(&mut self) -> Result<u32, Skip> {
        let Some(sig) = self.frame_sig()? else { return Ok(0) };
        let Some(rec) = &self.bmi.recorder else { return Err(SKIP_INST_KEY) };
        if let Some(&c) = rec.ent_ix.get(&sig) {
            return c.ok_or(SKIP_INST_KEY);
        }
        let key = self.frame_key(None, sig);
        let Some(rec) = &mut self.bmi.recorder else { return Err(SKIP_INST_KEY) };
        let id = key.map(|k| {
            if let Some(&i) = rec.inst_index.get(&k) {
                return i;
            }
            rec.insts.push(k.clone());
            let i = rec.insts.len() as u32;
            rec.inst_index.insert(k, i);
            i
        });
        rec.ent_ix.insert(sig, id);
        id.ok_or(SKIP_INST_KEY)
    }

    /// Table `t`'s [`EvalKey::inst`] for an evaluation made now, or
    /// `None` when the table carries nothing for the frame.
    fn table_inst(&mut self, t: usize) -> Option<u32> {
        let sig = match self.frame_sig() {
            Ok(None) => return Some(0),
            Ok(Some(sig)) => sig,
            Err(_) => return None,
        };
        let table = &self.bmi.tables[t];
        if table.insts.is_empty() {
            return None;
        }
        if let Some(&c) = table.ent_ix.get(&sig) {
            return c;
        }
        let key = self.frame_key(Some(t), sig);
        let table = &mut self.bmi.tables[t];
        let id = key.and_then(|k| table.insts.get(&k).copied());
        table.ent_ix.insert(sig, id);
        id
    }

    /// The carried result of evaluating `e` in `ctx`, if the unit being
    /// read came from a kind-2 module file that holds one which fits.
    pub(crate) fn carried_lookup(&mut self, e: ExprId, ctx: &EvalContext) -> Option<ConstValueId> {
        if self.bmi.tables.is_empty() {
            return None;
        }
        let unit = self.module_current_unit();
        let own = self.bmi.tables.iter().position(|t| t.unit == unit);
        if let Some(t) = own
            && let Some(v) = self.carried_lookup_in(t, e, ctx, true)
        {
            return Some(v);
        }
        // Inside an instantiation the key names it, so another unit's
        // table may hold the result: a unit's pending instantiations run
        // at the importer's end of TU, after the replay.
        if !matches!(self.frame_sig(), Ok(Some(_))) {
            return None;
        }
        (0..self.bmi.tables.len())
            .filter(|&t| Some(t) != own)
            .find_map(|t| self.carried_lookup_in(t, e, ctx, false))
    }

    /// The result table `t` carries for evaluating `e` in `ctx`; outside
    /// the table's own unit (`own` false) only inside an instantiation.
    fn carried_lookup_in(
        &mut self,
        t: usize,
        e: ExprId,
        ctx: &EvalContext,
        own: bool,
    ) -> Option<ConstValueId> {
        let inst = self.table_inst(t)?;
        if inst == 0 && !own {
            return None;
        }
        let key = self.eval_key(e, ctx, Some(t), inst)?;
        let v = self.bmi.tables[t].map.get(&key)?.clone();
        let ty = self.carried_type(e);
        let id = self.intern_carried(t, ty, &v)?;
        self.bmi.hits += 1;
        Some(id)
    }

    /// The canonical, unqualified type a value of reference type `t`
    /// refers to, or `None` when `t` is not a reference.
    pub(super) fn carried_referee(&mut self, t: QualType) -> Option<QualType> {
        if !matches!(
            self.ast.types.canonical_view(t),
            TypeView::LValueRef(_) | TypeView::RValueRef(_)
        ) {
            return None;
        }
        let p = self.ast.types.pointee(t)?;
        let c = self.ast.types.canonical(p);
        Some(self.ast.types.unqualified(c))
    }

    /// The canonical, unqualified type of `e`.
    fn carried_type(&mut self, e: ExprId) -> QualType {
        let q = self.ast.exprs.get(e).ty();
        let c = self.ast.types.canonical(q);
        self.ast.types.unqualified(c)
    }

    /// Interns `v`, from table `t`, as a value of type `ty` (canonical),
    /// or `None` when it does not fit that type.
    fn intern_carried(&mut self, t: usize, ty: QualType, v: &Carried) -> Option<ConstValueId> {
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
            Carried::Wire(b) => return self.intern_wire(t, ty, b),
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
        let inst = match self.record_inst() {
            Ok(i) => i,
            Err(why) => {
                self.carried_skip(why);
                return None;
            }
        };
        let Some(key) = self.eval_key(e, ctx, None, inst) else {
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
                } else {
                    let ty = self.carried_type(e);
                    let vt = self.ast.consts.ty(*v);
                    if vt == ty {
                        self.record_value(*v, false)
                    } else if self.carried_referee(vt) == Some(ty) {
                        // A reference's value: its type is the reference's.
                        self.record_value(*v, true)
                    } else {
                        Err("a value whose type is not its expression's")
                    }
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
            // The same evaluation made again (a constraint checked twice)
            // keeps its result; two results at one key carry neither.
            let old = &mut rec.entries[i].1;
            if old.is_some() && *old != carried {
                *old = None;
                rec.first_skip.get_or_insert("two evaluations at one position");
            }
            return;
        }
        rec.index.insert(p.key, rec.entries.len());
        rec.entries.push((p.key, carried));
    }
}

impl Sema<'_, '_> {
    /// `v` as the writer carries it, or why it cannot; `typed` when `v`
    /// is a reference's value, whose type the entry then names.
    fn record_value(&mut self, v: ConstValueId, typed: bool) -> Result<Carried, Skip> {
        let why = match carried_value(&self.ast.consts, v) {
            Ok(c) if !typed => return Ok(c),
            Ok(_) => "a reference value",
            Err(why) => why,
        };
        let sm = self.diags.sm();
        let bmi = &mut self.bmi;
        let file = |f| file_index(bmi, None, sm, f);
        let oven = self.oven.as_deref();
        inst::value_bytes(&self.ast, oven, sm, file, v, typed)
            .map(Carried::Wire)
            .map_err(|w| w.unwrap_or(why))
    }
}

/// `v` as a carried value of part 1, or why it is not one (a value
/// [`inst::value_bytes`] may still write: the reason is then why that
/// could not).
fn carried_value(store: &ConstStore, v: ConstValueId) -> Result<Carried, Skip> {
    if let Some(s) = store.scalar(v) {
        return match s {
            Scalar::Int { signed, bits, .. } => {
                Ok(Carried::Int { signed, width: bits.width(), words: bits.words().into() })
            }
            Scalar::Bool { value, .. } => Ok(Carried::Bool(value)),
            Scalar::Float { kind, bits, .. } => Ok(Carried::Float { kind, bits }),
            Scalar::Null { kind: NullKind::Pointer, .. } => Ok(Carried::NullPointer),
            Scalar::Null { .. } | Scalar::MemberPtr { .. } => {
                Err("a pointer-to-member value whose member has no key")
            }
            Scalar::Pointer { .. } => Err("a pointer or reference value whose object has no key"),
            Scalar::Reflection { .. } => Err("a reflection value with no key"),
        };
    }
    let Some(img) = store.aggregate(v) else { return Err("an unknown value") };
    if !img.relocs.is_empty() || !img.unions.is_empty() {
        return Err("an aggregate holding a value with no key");
    }
    Ok(Carried::Aggregate { bytes: img.bytes.into(), padding: img.padding.into() })
}

// --- encoding ---------------------------------------------------------

const TAG_INT: u8 = 0;
const TAG_BOOL: u8 = 1;
const TAG_FLOAT: u8 = 2;
const TAG_NULL: u8 = 3;
const TAG_AGGREGATE: u8 = 4;
/// A null pointer to member (its kind follows).
const TAG_MEMBER_NULL: u8 = 5;
const TAG_POINTER: u8 = 6;
const TAG_MEMBER_POINTER: u8 = 7;
const TAG_REFLECTION: u8 = 8;
/// An aggregate with relocations or union members.
const TAG_AGGREGATE_EX: u8 = 9;
/// A whole entry's value whose type is a reference to its expression's.
const TAG_TYPED: u8 = 10;

/// The first word of a versioned section. A version-1 section (WP-985)
/// starts with its path count, which is never this.
const VERSIONED: u32 = u32::MAX;

/// The section format version written (spec/modules.md §11.11
/// "Encoding"): 2 adds instantiation keys, 3 the values of tags 5 to 9
/// (WP-1047), 4 string literal, temporary and label pointer bases and
/// alias template specialization, data member specification and
/// annotation reflections (WP-1160).
const VERSION: u32 = 4;

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// An unsigned LEB128 varint.
fn put_var(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_u32(out, u32::try_from(b.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(b);
}

fn put_value(out: &mut Vec<u8>, v: &Carried) {
    match v {
        Carried::Int { signed, width, words } => {
            out.push(TAG_INT);
            out.push(u8::from(*signed));
            put_u32(out, *width);
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
        Carried::Wire(b) => out.extend_from_slice(b),
        Carried::Aggregate { bytes, padding } => {
            out.push(TAG_AGGREGATE);
            put_bytes(out, bytes);
            put_u32(out, padding.len() as u32);
            for r in padding {
                put_u32(out, r.start);
                put_u32(out, r.len);
            }
        }
    }
}

/// The section bytes of `rec`: the version, the path table, the
/// instantiation keys the carried entries name (renumbered in first-use
/// order), then each carried entry in first-evaluation order
/// (deterministic: no hash-map iteration).
fn encode(rec: &Recorder) -> CarriedSection {
    let kept: Vec<(&EvalKey, &Carried)> =
        rec.entries.iter().filter_map(|(k, v)| v.as_ref().map(|v| (k, v))).collect();
    if kept.is_empty() {
        return CarriedSection {
            bytes: Vec::new(),
            carried: 0,
            inst_carried: 0,
            first_skip: rec.first_skip,
        };
    }
    // `renum[i]`: the written index + 1 of recorder key i, 0 = unused.
    let mut renum = vec![0u32; rec.insts.len()];
    let mut insts: Vec<&[u8]> = Vec::new();
    let mut inst_carried = 0;
    for (k, _) in &kept {
        if k.inst != 0 {
            inst_carried += 1;
            let slot = &mut renum[k.inst as usize - 1];
            if *slot == 0 {
                insts.push(&rec.insts[k.inst as usize - 1]);
                *slot = insts.len() as u32;
            }
        }
    }
    let mut out = Vec::new();
    put_u32(&mut out, VERSIONED);
    put_u32(&mut out, VERSION);
    put_u32(&mut out, rec.paths.len() as u32);
    for p in &rec.paths {
        put_bytes(&mut out, p.as_bytes());
    }
    put_u32(&mut out, insts.len() as u32);
    for k in &insts {
        put_bytes(&mut out, k);
    }
    put_u32(&mut out, kept.len() as u32);
    for (k, v) in &kept {
        put_u32(&mut out, if k.inst == 0 { 0 } else { renum[k.inst as usize - 1] });
        out.push(k.kind);
        for w in k.at.0.iter().chain(&k.expr.0) {
            put_u32(&mut out, *w);
        }
        put_value(&mut out, v);
    }
    CarriedSection { bytes: out, carried: kept.len(), inst_carried, first_skip: rec.first_skip }
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
    /// An unsigned LEB128 varint of at most 64 bits.
    fn var(&mut self) -> Res<u64> {
        let mut v = 0u64;
        for i in 0..10 {
            let b = self.u8()?;
            if i == 9 && b > 1 {
                return bad("varint overflow");
            }
            v |= u64::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        bad("varint overflow")
    }
    /// A varint count, checked as [`Rd::count`].
    fn var_count(&mut self, min: usize) -> Res<usize> {
        let n = usize::try_from(self.var()?).unwrap_or(usize::MAX);
        if n.saturating_mul(min.max(1)) > self.0.len() {
            return bad("length exceeds the section");
        }
        Ok(n)
    }
}

/// Reads one carried value whose positions name paths below `npaths`.
fn read_value(r: &mut Rd<'_>, npaths: usize) -> Res<Carried> {
    if r.0.first().is_some_and(|&t| t <= TAG_AGGREGATE) {
        return read_plain(r);
    }
    let start = r.0;
    inst::check_value(r, npaths)?;
    Ok(Carried::Wire(start[..start.len() - r.0.len()].into()))
}

/// Reads one value of tags 0 to 4 (part 1).
fn read_plain(r: &mut Rd<'_>) -> Res<Carried> {
    Ok(match r.u8()? {
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
    })
}

/// The smallest encoded entry: instantiation index (version 2), kind,
/// two keys, a tag.
const MIN_ENTRY_V1: usize = 1 + 32 + 1;
const MIN_ENTRY: usize = 4 + MIN_ENTRY_V1;

/// A decoded section.
#[derive(Debug, PartialEq)]
struct Decoded {
    paths: Vec<String>,
    /// Instantiation key -> [`EvalKey::inst`].
    insts: FxHashMap<Box<[u8]>, u32>,
    map: FxHashMap<EvalKey, Carried>,
}

/// Decodes a carried-constants section of version 1, 2 or 3. Total: any
/// malformed input is a [`SectionError`].
fn decode(bytes: &[u8]) -> Res<Decoded> {
    let mut r = Rd(bytes);
    let v2 = bytes.get(..4) == Some(&VERSIONED.to_le_bytes()[..]);
    if v2 {
        r.take(4)?;
        if !(2..=VERSION).contains(&r.u32()?) {
            return bad("unsupported section version");
        }
    }
    let npaths = r.count(4)?;
    let mut paths = Vec::with_capacity(npaths);
    for _ in 0..npaths {
        match std::str::from_utf8(r.bytes()?) {
            Ok(s) => paths.push(s.to_owned()),
            Err(_) => return bad("invalid UTF-8 in a path"),
        }
    }
    let mut insts = FxHashMap::default();
    if v2 {
        let n = r.count(5)?;
        insts.reserve(n);
        for i in 0..n {
            let k = r.bytes()?;
            inst::check_key(k, paths.len())?;
            if insts.insert(k.into(), i as u32 + 1).is_some() {
                return bad("duplicate instantiation key");
            }
        }
    }
    let n = r.count(if v2 { MIN_ENTRY } else { MIN_ENTRY_V1 })?;
    if n == 0 {
        return bad("no entries");
    }
    let mut map = FxHashMap::default();
    map.reserve(n);
    for _ in 0..n {
        let inst = if v2 { r.u32()? } else { 0 };
        if inst as usize > insts.len() {
            return bad("instantiation index out of range");
        }
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
            inst,
            kind,
            at: LocKey([w[0], w[1], w[2], w[3]]),
            expr: LocKey([w[4], w[5], w[6], w[7]]),
        };
        let v = read_value(&mut r, paths.len())?;
        if map.insert(key, v).is_some() {
            return bad("duplicate entry");
        }
    }
    if !r.0.is_empty() {
        return bad("trailing bytes");
    }
    Ok(Decoded { paths, insts, map })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed instantiation key: `S`, a declared template named
    /// `name` at offset `off` of path 0, and the arguments `<int-ish
    /// builtin 5, (builtin 7) n>`.
    fn inst_key(name: &str, off: u32, n: u32) -> Box<[u8]> {
        let mut k = vec![b'S', b'D', 3, 0];
        put_bytes(&mut k, name.as_bytes());
        for w in [0, off, 0, off] {
            put_u32(&mut k, w);
        }
        put_u32(&mut k, 2);
        k.extend_from_slice(&[b'T', 0, 0, 5, b'V', 0, 0, 7]);
        put_value(
            &mut k,
            &Carried::Int { signed: true, width: 32, words: vec![u64::from(n)].into() },
        );
        k.into()
    }

    fn sample() -> Recorder {
        let key = |n: u32, inst: u32| EvalKey {
            inst,
            kind: 1,
            at: LocKey([0, n, 0, n]),
            expr: LocKey([1, n, 0, 7]),
        };
        let mut r = Recorder { paths: vec!["m.cppm".into(), "h.h".into()], ..Recorder::default() };
        // Key 1 is named only by an entry that is not carried, so it is
        // not written and keys 0 and 2 are renumbered.
        r.insts = vec![inst_key("A", 3, 1), inst_key("B", 9, 2), inst_key("A", 3, 2)];
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
            r.entries.push((key(i as u32, [0, 3, 1][i % 3]), Some(v)));
        }
        r.entries.push((key(6, 1), Some(Carried::Bool(false))));
        r.entries.push((key(99, 0), None));
        r.entries.push((key(99, 2), None));
        r
    }

    /// Every value kind round-trips, with and without an instantiation
    /// key, the encoding is deterministic, and entries (and keys) that are
    /// not carried are not written.
    #[test]
    fn round_trip() {
        let rec = sample();
        let s = encode(&rec);
        assert_eq!((s.carried, s.inst_carried), (7, 5));
        assert_eq!(encode(&rec).bytes, s.bytes, "deterministic");
        let d = decode(&s.bytes).unwrap();
        assert_eq!(d.paths, rec.paths);
        assert_eq!(d.map.len(), 7);
        assert_eq!(d.insts.len(), 2, "the unused key is not written");
        for (k, v) in &rec.entries {
            let Some(v) = v else { continue };
            let inst = if k.inst == 0 { 0 } else { d.insts[&rec.insts[k.inst as usize - 1]] };
            assert_eq!(d.map.get(&EvalKey { inst, ..*k }), Some(v));
        }
    }

    /// A version-1 section (WP-985, no instantiation keys) still reads.
    #[test]
    fn reads_version_1() {
        let mut b = Vec::new();
        put_u32(&mut b, 1);
        put_bytes(&mut b, b"m.cppm");
        put_u32(&mut b, 1);
        b.push(4);
        for w in [0, 10, 0, 10, 0, 12, 0, 12] {
            put_u32(&mut b, w);
        }
        put_value(&mut b, &Carried::Bool(true));
        let d = decode(&b).unwrap();
        assert!(d.insts.is_empty());
        let key =
            EvalKey { inst: 0, kind: 4, at: LocKey([0, 10, 0, 10]), expr: LocKey([0, 12, 0, 12]) };
        assert_eq!(d.map.get(&key), Some(&Carried::Bool(true)));
    }

    #[test]
    fn empty_is_not_written() {
        let s = encode(&Recorder::default());
        assert!(s.bytes.is_empty());
        assert!(decode(&s.bytes).is_err());
    }

    fn damage(b: &[u8], f: impl Fn(&[u8])) {
        for i in 0..b.len() {
            for x in [0x01u8, 0x80, 0xff, 0x7f, 0x10] {
                let mut m = b.to_vec();
                m[i] ^= x;
                f(&m);
            }
        }
        for i in 0..b.len().saturating_sub(4) {
            for v in [u32::MAX, u32::MAX / 2, 0x0100_0000, 0] {
                let mut m = b.to_vec();
                m[i..i + 4].copy_from_slice(&v.to_le_bytes());
                f(&m);
            }
        }
        // A deterministic pseudo-random walk of multi-byte damage.
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..4000 {
            let mut m = b.to_vec();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = (seed % m.len() as u64) as usize;
                m[i] = (seed >> 32) as u8;
            }
            f(&m);
        }
    }

    /// Every truncation, every single-byte change and every length field
    /// set to large values decodes to a value or a `SectionError`, never a
    /// panic; truncations are always errors. The same for instantiation
    /// keys on their own.
    #[test]
    fn fuzz_truncate_flip_lengths() {
        let b = encode(&sample()).bytes;
        for n in 0..b.len() {
            assert!(decode(&b[..n]).is_err(), "truncated to {n}");
        }
        damage(&b, |m| {
            let _ = decode(m);
        });
        let k = inst_key("A", 3, 1);
        assert_eq!(inst::check_key(&k, 1), Ok(()));
        for n in 0..k.len() {
            assert!(inst::check_key(&k[..n], 1).is_err(), "key truncated to {n}");
        }
        damage(&k, |m| {
            let _ = inst::check_key(m, 1);
        });
    }

    /// Deeply nested keys are rejected without deep recursion.
    #[test]
    fn rejects_deep_keys() {
        let deep = vec![b'S'; 100_000];
        assert!(inst::check_key(&deep, 1).is_err());
        let mut packs = vec![b'S', b'D', 0, 2];
        packs.extend_from_slice(&[0; 16]);
        for _ in 0..10_000 {
            put_u32(&mut packs, 1);
            packs.push(b'K');
        }
        assert!(inst::check_key(&packs, 1).is_err());
    }

    /// The declaration entity `name` at offset `off` of path 0.
    fn decl_ent(k: &mut Vec<u8>, name: &str, off: u32) {
        k.extend_from_slice(&[b'D', 3, 0]);
        put_bytes(k, name.as_bytes());
        for w in [0, off, 0, off] {
            put_u32(k, w);
        }
    }

    /// One hand-built value of each WP-1047 tag (spec/modules.md §11.11
    /// "Encoding"), as an entry's bytes.
    fn wire_values() -> Vec<Vec<u8>> {
        use inst::{
            BASE_CONST, BASE_DECL, BASE_TYPE_INFO, REFL_ALIAS, REFL_TYPE, TY_LREF, TY_POINTER,
        };
        let int = [0u8, 0, 5];
        let mut out = Vec::new();
        // &g + 300, one past the end, designating an int.
        let mut v = vec![TAG_POINTER, 3, BASE_DECL];
        decl_ent(&mut v, "g", 5);
        put_var(&mut v, 300);
        v.extend_from_slice(&int);
        out.push(v.clone());
        let pointer = v;
        // &typeid(int), and a pointer into an int constant.
        out.push([&[TAG_POINTER, 0, BASE_TYPE_INFO][..], &int, &[0]].concat());
        let mut v = [&[TAG_POINTER, 0, BASE_CONST][..], &int].concat();
        put_value(&mut v, &Carried::Int { signed: true, width: 32, words: vec![7].into() });
        v.push(0);
        out.push(v);
        // &D::b through a one-step derivation path.
        let mut v = vec![TAG_MEMBER_POINTER, 2];
        decl_ent(&mut v, "b", 9);
        put_var(&mut v, 1);
        decl_ent(&mut v, "D", 11);
        out.push(v);
        out.push(vec![TAG_MEMBER_NULL, 2]);
        // Reflections: a type, an alias, a declaration, null, a value and
        // a base.
        out.push([&[TAG_REFLECTION, 0, REFL_TYPE][..], &int].concat());
        let mut v = vec![TAG_REFLECTION, 0, REFL_ALIAS];
        decl_ent(&mut v, "A", 13);
        out.push(v);
        let mut v = vec![TAG_REFLECTION, 1];
        decl_ent(&mut v, "S", 15);
        out.push(v);
        out.push(vec![TAG_REFLECTION, 2]);
        let mut v = [&[TAG_REFLECTION, 6][..], &int].concat();
        put_value(&mut v, &Carried::Int { signed: true, width: 32, words: vec![42].into() });
        out.push(v);
        let mut v = vec![TAG_REFLECTION, 8];
        decl_ent(&mut v, "D", 11);
        put_var(&mut v, 0);
        out.push(v);
        // An aggregate { int *p; int n; } with a relocation, padding and a
        // union member.
        let mut v = vec![TAG_AGGREGATE_EX];
        put_bytes(&mut v, &[0; 16]);
        put_var(&mut v, 1);
        put_var(&mut v, 12);
        put_var(&mut v, 4);
        put_var(&mut v, 1);
        put_var(&mut v, 0);
        v.extend_from_slice(&[0, TY_POINTER, 0, 0, 5]);
        v.extend_from_slice(&pointer);
        put_var(&mut v, 1);
        put_var(&mut v, 8);
        decl_ent(&mut v, "u", 17);
        out.push(v);
        // A reference's value.
        out.push([&[TAG_TYPED, 0, TY_LREF][..], &int, &pointer].concat());
        out
    }

    /// A recorder of [`wire_values`] entries.
    fn wire_sample() -> Recorder {
        let mut r = Recorder { paths: vec!["m.cppm".into()], ..Recorder::default() };
        r.insts = vec![wire_key()];
        for (i, v) in wire_values().into_iter().enumerate() {
            let at = LocKey([0, i as u32, 0, i as u32]);
            let key = EvalKey { inst: (i % 2) as u32, kind: 1, at, expr: at };
            r.entries.push((key, Some(Carried::Wire(v.into()))));
        }
        r
    }

    /// An instantiation key with a pointer and a reflection argument.
    fn wire_key() -> Box<[u8]> {
        use inst::{TY_BUILTIN, TY_POINTER};
        let vals = wire_values();
        let mut k = vec![b'S'];
        decl_ent(&mut k, "X", 3);
        put_u32(&mut k, 2);
        k.extend_from_slice(&[b'V', 0, TY_POINTER, 0, 0, 5]);
        k.extend_from_slice(&vals[0]);
        k.extend_from_slice(&[b'V', 0, TY_BUILTIN, BuiltinType::MetaInfo as u8]);
        k.extend_from_slice(&vals[7]);
        k.into()
    }

    /// Pointer, member pointer, reflection, aggregate and reference values
    /// (WP-1047) round-trip byte for byte and keep the older values'
    /// bytes; each checks on its own and no truncation of one does.
    #[test]
    fn wire_round_trip() {
        let rec = wire_sample();
        let s = encode(&rec);
        let d = decode(&s.bytes).unwrap();
        assert_eq!(d.map.len(), rec.entries.len());
        for (k, v) in &rec.entries {
            let inst = if k.inst == 0 { 0 } else { d.insts[&rec.insts[0]] };
            assert_eq!(d.map.get(&EvalKey { inst, ..*k }), v.as_ref());
        }
        for v in wire_values() {
            let mut r = Rd(&v);
            assert_eq!(inst::check_value(&mut r, 1), Ok(()));
            assert!(r.0.is_empty());
            for n in 0..v.len() {
                assert!(inst::check_value(&mut Rd(&v[..n]), 1).is_err(), "truncated to {n}");
            }
        }
        // A pointer to an entity at a path the section lacks.
        assert!(inst::check_value(&mut Rd(&wire_values()[0]), 0).is_err());
        let mut old = Vec::new();
        put_value(&mut old, &Carried::Bool(true));
        assert_eq!(old, [TAG_BOOL, 1], "older values keep their bytes");
        assert_eq!(inst::check_key(&wire_key(), 1), Ok(()));
    }

    /// The same damage as [`fuzz_truncate_flip_lengths`], over WP-1047
    /// values and keys.
    #[test]
    fn fuzz_wire_values() {
        let b = encode(&wire_sample()).bytes;
        for n in 0..b.len() {
            assert!(decode(&b[..n]).is_err(), "truncated to {n}");
        }
        damage(&b, |m| {
            let _ = decode(m);
        });
        let k = wire_key();
        for n in 0..k.len() {
            assert!(inst::check_key(&k[..n], 1).is_err(), "key truncated to {n}");
        }
        damage(&k, |m| {
            let _ = inst::check_key(m, 1);
        });
        // Overlong and overflowing varints.
        let mut r = Rd(&[0x80; 11]);
        assert!(r.var().is_err());
        let mut r = Rd(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02]);
        assert!(r.var().is_err());
        let mut v = Vec::new();
        put_var(&mut v, u64::MAX);
        assert_eq!(Rd(&v).var(), Ok(u64::MAX));
    }

    #[test]
    fn rejects_malformed() {
        let b = encode(&sample()).bytes;
        let mut trailing = b.clone();
        trailing.push(0);
        assert_eq!(
            decode(&trailing),
            Err(SectionError("carried constants: trailing bytes".into()))
        );
        let mut v4 = b.clone();
        v4[4..8].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(decode(&v4).unwrap_err(), bad::<()>("unsupported section version").unwrap_err());
        let paths_len = 4 + (4 + 6) + (4 + 3);
        let insts_at = 8 + paths_len;
        // The first key's entity tag.
        let mut m = b.clone();
        m[insts_at + 4 + 4] = b'Z';
        assert_eq!(decode(&m).unwrap_err(), bad::<()>("bad instantiation key").unwrap_err());
        let k = inst_key("A", 3, 1).len();
        let entries_at = insts_at + 4 + 2 * (4 + k) + 4;
        let mut m = b.clone();
        m[entries_at..entries_at + 4].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(
            decode(&m).unwrap_err(),
            bad::<()>("instantiation index out of range").unwrap_err()
        );
        // The first entry's `at` path index.
        let mut m = b.clone();
        m[entries_at + 5..entries_at + 9].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(decode(&m).unwrap_err(), bad::<()>("path index out of range").unwrap_err());
        // A key whose position names a path the section lacks.
        let mut m = b.clone();
        let at_path = insts_at + 4 + 4 + 4 + 4 + 1;
        m[at_path..at_path + 4].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(decode(&m).unwrap_err(), bad::<()>("path index out of range").unwrap_err());
    }
}
