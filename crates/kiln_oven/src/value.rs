//! Freezing results into the `ConstStore`, thawing stored constants into
//! static memory, and resolving globals (namespace-scope variables and
//! string literals) on first use.

use kiln_ast::consts::{
    ByteImage, ByteRange, ConstValue, IntBits, NullKind, PtrBase, Reloc, Scalar, UnionMember,
};
use kiln_ast::decl::VarEvalState;
use kiln_ast::strings::StringEncoding;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, StringLitId};
use kiln_source::Loc;

use crate::compile::{GKind, GState, RetKind, is_local_var};
use crate::failure::{EvalFailure, FailKind, Note};
use crate::mem::{Origin, Ptr, flags, fn_ptr};
use crate::ty::{OTy, TyKind};
use crate::vm::{Counters, value_len};
use crate::{Oven, SemaBridge};

/// Cap on nested global initializations evaluated on demand (a deeper
/// chain reads the variable as unknown).
const MAX_NESTING: u32 = 512;

/// Reusable buffers of the freezer (no allocation per evaluation once
/// warm).
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    bytes: Vec<u8>,
    cover: Vec<u8>,
    relocs: Vec<Reloc>,
    padding: Vec<ByteRange>,
    unions: Vec<UnionMember>,
}

impl Scratch {
    /// Heap bytes held (see `Oven::footprint`).
    pub(crate) fn footprint(&self) -> usize {
        self.bytes.capacity()
            + self.cover.capacity()
            + self.relocs.capacity() * size_of::<Reloc>()
            + self.padding.capacity() * size_of::<ByteRange>()
            + self.unions.capacity() * size_of::<UnionMember>()
    }
}

/// How a variable reads in constant expressions.
pub(crate) struct VarInfo {
    /// Usable in constant expressions ([expr.const]/4): constexpr, or a
    /// const-qualified integral or enumeration type.
    pub usable: bool,
    pub init: Option<ExprId>,
    pub state: VarEvalState,
    pub ty: QualType,
    pub is_const: bool,
}

pub(crate) fn var_info(ast: &Ast, d: DeclId) -> VarInfo {
    let mut info = VarInfo {
        usable: false,
        init: None,
        state: VarEvalState::NotEvaluated,
        ty: ast.decls.get(d).ty().unwrap_or(QualType::INT),
        is_const: false,
    };
    let mut constexpr = false;
    for r in ast.redecls_of(d) {
        if let Some(v) = ast.decls.var(r) {
            constexpr |= v.spec.is_constexpr();
            if info.init.is_none() && v.init.is_some() {
                info.init = v.init;
                info.state = v.eval.state();
                info.ty = ast.decls.get(r).ty().unwrap_or(info.ty);
            }
        }
    }
    let c = ast.types.canonical(info.ty);
    info.is_const = c.has_const_bit();
    let integral = ast.types.is_integral(c) || ast.types.as_enum(c).is_some();
    // Potentially-constant ([expr.const]/3): constexpr, a reference, or
    // const-qualified integral or enumeration type.
    info.usable = constexpr || ast.types.is_reference(c) || (info.is_const && integral);
    info
}

/// Why a value cannot be frozen.
enum Freeze {
    Fault(FailKind),
    Notes(Vec<Note>),
}

impl Oven {
    /// A top-level evaluation with registers from `fb`: `object` is the
    /// variable initialized.
    pub(crate) fn eval_inner(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.run.object = object;
        let mut pre = None;
        if let Some(d) = object {
            let ast = b.ast();
            if !is_local_var(ast, d) && ast.decls.var(d).is_some() {
                let g = self.global_index(ast, d);
                if self.globals[g as usize].state == GState::Unresolved {
                    let q = ast.decls.get(d).ty().unwrap_or(QualType::INT);
                    let ty = self.types.intern(b, q);
                    let p = self.placeholder(g, ty, ast_first(b, d));
                    self.globals[g as usize].state = GState::Resolved(p);
                    pre = Some(g);
                }
            }
        }
        let r = self.eval_at(b, e, object, fb);
        if let Some(g) = pre {
            self.globals[g as usize].state = GState::Unresolved;
        }
        r
    }

    /// The static storage of global `g` while its initializer runs (reused
    /// by later evaluations of the same variable).
    fn placeholder(&mut self, g: u32, ty: OTy, d: DeclId) -> u64 {
        let info = *self.types.info(ty);
        let fl =
            flags::CONST | flags::RESOLVING | if info.has_union { flags::HAS_UNION } else { 0 };
        let slot = self.globals[g as usize].slot;
        if slot != 0 {
            let p = Ptr::decode(slot);
            let a = self.mem.alloc_of_mut(p);
            if u64::from(a.size) == info.size && a.ty == ty {
                a.flags = fl;
                a.origin = Origin::Global(d);
                let (base, size) = (u64::from(a.base), info.size);
                self.mem.stat.set_init(base, size, false);
                self.mem.stat.set_ptr(base, size, false);
                return slot;
            }
        }
        let p = self.mem.alloc_static(ty, info.size, fl, Origin::Global(d));
        self.globals[g as usize].slot = p;
        p
    }

    /// Compiles and runs `e` with its frame at `fb` and freezes the result.
    pub(crate) fn eval_at(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        let f = self.compile_thunk(b, e, object);
        self.let_pending.clear();
        self.exec(b, f, fb)?;
        let v = self.regs[fb as usize];
        // clang's `CheckConstantExpression` location: the variable for an
        // initializer (`EvaluateAsInitializer`), else the expression.
        let loc = match object {
            Some(d) => b.ast().decls.get(d).loc(),
            None => b.ast().exprs.get(e).loc(),
        };
        let r = match self.funcs[f as usize].ret {
            RetKind::Void => {
                Err(Freeze::Notes(vec![Note::new(loc, "expression of type 'void' has no value")]))
            }
            RetKind::Scalar(ty) => self.freeze_scalar(b, ty, v, loc),
            RetKind::Sret(ty) => self.freeze_aggregate(b, ty, v as u64, loc),
        };
        // [expr.const]/5.20: every allocation is deallocated within the
        // evaluation (checked after the result, as clang).
        let r = r.and_then(|cv| match self.leak_notes(b) {
            None => Ok(cv),
            Some(n) => Err(Freeze::Notes(n)),
        });
        let cv = r.map_err(|f| match f {
            Freeze::Fault(k) => EvalFailure::silent(k),
            Freeze::Notes(n) if self.run.silent => {
                drop(n);
                EvalFailure::silent(FailKind::NotConstant)
            }
            Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
        })?;
        // [dcl.constexpr]/6: a `constexpr` variable has constant
        // destruction ([expr.const]/7), checked on the object just built.
        if let (Some(d), RetKind::Sret(ty)) = (object, self.funcs[f as usize].ret)
            && !self.run.silent
            && b.ast().decls.var(d).is_some_and(|x| x.spec.is_constexpr())
            && let Some(df) = self.compile_destroy(b, d, ty)
        {
            self.regs[fb as usize] = v;
            self.exec(b, df, fb)
                .map_err(|e| EvalFailure { kind: FailKind::Destruction, notes: e.notes })?;
        }
        self.freeze_lets(b, loc)?;
        Ok(cv)
    }

    /// Whether static temporary `t` is extended by the variable being
    /// initialized.
    fn extends_object(&self, ast: &Ast, t: DeclId) -> bool {
        let Some(l) = ast.decls.lifetime_extended(t) else { return false };
        self.run.object.is_some_and(|o| ast.first(o) == ast.first(l.extending))
    }

    /// Freezes the static temporaries the result points to (and those
    /// their values point to) into their `LetData.value`s.
    fn freeze_lets(&mut self, b: &mut dyn SemaBridge, loc: Loc) -> Result<(), EvalFailure> {
        let mut i = 0;
        let mut out = Vec::new();
        while i < self.let_pending.len() {
            let (t, p) = self.let_pending[i];
            i += 1;
            let r = self.freeze_object(b, p, loc).map_err(|f| match f {
                Freeze::Fault(k) => EvalFailure::silent(k),
                Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
            })?;
            out.push((t, r));
        }
        self.let_pending.clear();
        for (t, v) in out {
            b.set_temporary_value(t, v);
        }
        Ok(())
    }

    /// The value of the complete object at `p`.
    fn freeze_object(
        &mut self,
        b: &mut dyn SemaBridge,
        p: u64,
        loc: Loc,
    ) -> Result<ConstValueId, Freeze> {
        let Some(a) = self.alloc_any(Ptr::decode(p)).copied() else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        if self.types.is_aggregate(a.ty) {
            return self.freeze_aggregate(b, a.ty, p, loc);
        }
        let Ok(v) = self.load(p, a.ty, crate::kbc::ak::READ) else {
            return Err(self.uninit(b, None, a.ty, loc));
        };
        self.freeze_scalar(b, a.ty, v, loc)
    }

    /// clang's `note_constexpr_memory_leak` for the allocations of the
    /// running context still live: the first made, and how many others.
    fn leak_notes(&self, b: &dyn SemaBridge) -> Option<Vec<Note>> {
        if self.mem.heap_count == 0 {
            return None;
        }
        let mut first: Option<(u32, ExprId)> = None;
        let mut n = 0u64;
        for (_, a) in self.mem.heap_live_of(self.mem.ctx) {
            if let Origin::Heap(e, k) = a.origin {
                n += 1;
                if first.is_none_or(|(f, _)| k < f) {
                    first = Some((k, e));
                }
            }
        }
        let (_, e) = first?;
        if self.run.silent {
            return Some(Vec::new());
        }
        let others = match n - 1 {
            0 => String::new(),
            1 => " (along with 1 other memory leak)".into(),
            k => format!(" (along with {k} other memory leaks)"),
        };
        let loc = b.ast().exprs.get(e).loc();
        Some(vec![Note::new(loc, format!("allocation performed here was not deallocated{others}"))])
    }

    fn freeze_scalar(
        &mut self,
        b: &mut dyn SemaBridge,
        ty: OTy,
        v: u128,
        loc: Loc,
    ) -> Result<ConstValueId, Freeze> {
        if let TyKind::MemPtr { .. } = self.types.kind(ty) {
            return self.mp_freeze(b, ty, v).map_err(|_| Freeze::Fault(FailKind::MemoryLimit));
        }
        let s = self.scalar_of(b, ty, v, loc)?;
        b.consts().push_scalar(s).map_err(|_| Freeze::Fault(FailKind::MemoryLimit))
    }

    /// The store form of a scalar of type `ty` with register value `v`.
    fn scalar_of(
        &mut self,
        b: &mut dyn SemaBridge,
        ty: OTy,
        v: u128,
        loc: Loc,
    ) -> Result<Scalar<'static>, Freeze> {
        let info = *self.types.info(ty);
        Ok(match info.kind {
            TyKind::Bool => Scalar::Bool { ty: info.q, value: v != 0 },
            TyKind::Int { width, signed } => {
                let w = u32::from(width);
                let m = if w >= 128 { v } else { v & ((1u128 << w) - 1) };
                Scalar::Int { ty: info.q, signed, bits: IntBits::from_u128(w, m) }
            }
            TyKind::Float(kind) => {
                let bits = value_len(info.size, info.kind) * 8;
                let m = if bits >= 128 { v } else { v & ((1u128 << bits) - 1) };
                Scalar::Float { ty: info.q, kind, bits: m }
            }
            TyKind::NullPtr => Scalar::Null { ty: info.q, kind: NullKind::Pointer },
            TyKind::Ptr(pointee) => self.ptr_of(b, info.q, pointee, v as u64, loc)?,
            _ => {
                return Err(Freeze::Notes(vec![Note::new(
                    loc,
                    "kiln: value of this type cannot be represented as a constant",
                )]));
            }
        })
    }

    fn ptr_of(
        &mut self,
        b: &mut dyn SemaBridge,
        q: QualType,
        pointee: OTy,
        raw: u64,
        loc: Loc,
    ) -> Result<Scalar<'static>, Freeze> {
        if raw == 0 {
            return Ok(Scalar::Null { ty: q, kind: NullKind::Pointer });
        }
        let p = Ptr::decode(raw);
        if p.is_fn() {
            return match self.funcs.get(p.off as usize).and_then(|f| f.decl) {
                Some(d) => Ok(Scalar::Pointer { ty: q, base: PtrBase::Function(d), offset: 0 }),
                None => Err(Freeze::Fault(FailKind::NotConstant)),
            };
        }
        let offset = u64::from(p.off);
        let Some(a) = self.alloc_any(p).copied() else {
            if p.is_heap() && !self.run.silent {
                // A deleted allocation whose slot was reused.
                let what = if offset != 0 || p.past { "subobject of " } else { "" };
                return Err(Freeze::Notes(vec![Note::new(
                    loc,
                    format!("pointer to {what}heap-allocated object is not a constant expression"),
                )]));
            }
            return Err(Freeze::Notes(vec![Note::new(
                loc,
                "pointer to an object outside its lifetime is not a constant expression",
            )]));
        };
        if p.is_static() {
            let base = match a.origin {
                Origin::Global(d) | Origin::External(d) => PtrBase::Decl(d),
                Origin::Str(lit, Some(origin)) => PtrBase::StringLit { lit, origin },
                Origin::Const(c) => PtrBase::Const(c),
                Origin::Let(t) => PtrBase::Temporary(t),
                _ => return Err(Freeze::Fault(FailKind::NotConstant)),
            };
            return Ok(Scalar::Pointer { ty: q, base, offset });
        }
        // A temporary extended by the variable being initialized
        // ([class.temporary]/6): its value is frozen with the result.
        if let Origin::Let(t) = a.origin
            && a.live
            && self.extends_object(b.ast(), t)
        {
            if !self.let_pending.iter().any(|&(x, _)| x == t) {
                self.let_pending.push((t, Ptr::with_off(raw, 0, false)));
            }
            return Ok(Scalar::Pointer { ty: q, base: PtrBase::Temporary(t), offset });
        }
        if let (Origin::Var(d), Some(o)) = (a.origin, self.run.object)
            && d == o
            && a.live
        {
            let ast = b.ast();
            return Ok(Scalar::Pointer { ty: q, base: PtrBase::Decl(ast.first(d)), offset });
        }
        if self.run.silent {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        let ast = b.ast();
        let sub = offset != 0 || p.past || pointee != a.ty;
        let what = if sub { "subobject of " } else { "" };
        let mut notes = Vec::with_capacity(2);
        match a.origin {
            Origin::Var(d) => {
                let mut name = String::new();
                kiln_ast::dump::AstNames::new(ast).write_name(d, &mut name);
                notes.push(Note::new(
                    loc,
                    format!("pointer to {what}'{name}' is not a constant expression"),
                ));
                notes.push(Note::new(ast.decls.get(d).loc(), "declared here"));
            }
            Origin::Temp(e) => {
                notes.push(Note::new(
                    loc,
                    format!("pointer to {what}temporary is not a constant expression"),
                ));
                notes.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
            }
            Origin::Let(t) => {
                notes.push(Note::new(
                    loc,
                    format!("pointer to {what}temporary is not a constant expression"),
                ));
                if let Some(e) = let_mte(ast, t) {
                    notes.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
                }
            }
            // clang's note_constexpr_dynamic_alloc, before the leak check;
            // the allocation is noted while it is live.
            Origin::Heap(e, _) => {
                notes.push(Note::new(
                    loc,
                    format!("pointer to {what}heap-allocated object is not a constant expression"),
                ));
                if a.live {
                    notes.push(Note::new(ast.exprs.get(e).loc(), "heap allocation performed here"));
                }
            }
            _ => notes.push(Note::new(
                loc,
                "pointer to an object outside its lifetime is not a constant expression",
            )),
        }
        Err(Freeze::Notes(notes))
    }

    fn freeze_aggregate(
        &mut self,
        b: &mut dyn SemaBridge,
        ty: OTy,
        raw: u64,
        loc: Loc,
    ) -> Result<ConstValueId, Freeze> {
        let size = self.types.size(ty) as usize;
        let mut s = std::mem::take(&mut self.scratch);
        s.bytes.clear();
        s.bytes.resize(size, 0);
        s.cover.clear();
        s.cover.resize(size, 0);
        s.relocs.clear();
        s.padding.clear();
        s.unions.clear();
        let r = self.walk(b, &mut s, ty, raw, 0, None, loc);
        let out = r.and_then(|()| {
            s.relocs.sort_unstable_by_key(|r| r.at);
            s.unions.sort_unstable_by_key(|u| (u.at, u.field));
            let mut i = 0;
            while i < size {
                if s.cover[i] == 0 {
                    let start = i;
                    while i < size && s.cover[i] == 0 {
                        s.bytes[i] = 0;
                        i += 1;
                    }
                    s.padding.push(ByteRange { start: start as u32, len: (i - start) as u32 });
                } else {
                    i += 1;
                }
            }
            let img = ByteImage {
                ty: self.types.info(ty).q,
                bytes: &s.bytes,
                relocs: &s.relocs,
                padding: &s.padding,
                unions: &s.unions,
            };
            b.consts().push_aggregate(img).map_err(|_| Freeze::Fault(FailKind::NotConstant))
        });
        self.scratch = s;
        out
    }

    fn uninit(&self, b: &dyn SemaBridge, field: Option<DeclId>, ty: OTy, loc: Loc) -> Freeze {
        if self.run.silent {
            return Freeze::Fault(FailKind::NotConstant);
        }
        let ast = b.ast();
        let named = field.filter(|&f| !ast.decls.get(f).name().is_empty());
        if let Some(f) = named {
            let mut name = String::new();
            kiln_ast::dump::AstNames::new(ast).write_name(f, &mut name);
            Freeze::Notes(vec![
                Note::new(loc, format!("subobject '{name}' is not initialized")),
                Note::new(ast.decls.get(f).loc(), "subobject declared here"),
            ])
        } else {
            let t = Self::type_name(ast, self.types.info(ty).q);
            Freeze::Notes(vec![Note::new(
                loc,
                format!("subobject of type '{t}' is not initialized"),
            )])
        }
    }

    pub(crate) fn type_name(ast: &Ast, q: QualType) -> String {
        kiln_ast::types::print_type(&ast.types, ast, q, kiln_ast::types::PrintOptions::SUGARED)
    }

    /// Copies the object of type `t` at `raw` into the image at byte `at`.
    fn walk(
        &mut self,
        b: &mut dyn SemaBridge,
        s: &mut Scratch,
        t: OTy,
        raw: u64,
        at: usize,
        field: Option<DeclId>,
        loc: Loc,
    ) -> Result<(), Freeze> {
        let info = *self.types.info(t);
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p).copied() else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        let st = p.space();
        let abs = u64::from(a.base) + u64::from(p.off);
        match info.kind {
            TyKind::Bool | TyKind::Int { .. } | TyKind::Float(_) => {
                let len = value_len(info.size, info.kind);
                let sp = self.mem.space(st);
                if !sp.is_init(abs, len) {
                    return Err(self.uninit(b, field, t, loc));
                }
                let (a0, a1) = (abs as usize, (abs + len) as usize);
                s.bytes[at..at + len as usize].copy_from_slice(&sp.bytes[a0..a1]);
                s.cover[at..at + len as usize].fill(1);
            }
            TyKind::Ptr(_) | TyKind::NullPtr => {
                let sp = self.mem.space(st);
                if !sp.is_init(abs, info.size) {
                    return Err(self.uninit(b, field, t, loc));
                }
                let v = sp.read(abs, info.size);
                let sc = self.scalar_of(b, t, v, loc)?;
                let id =
                    b.consts().push_scalar(sc).map_err(|_| Freeze::Fault(FailKind::NotConstant))?;
                s.relocs.push(Reloc { at: at as u32, value: id });
                s.cover[at..at + info.size as usize].fill(1);
            }
            TyKind::MemPtr { .. } => {
                let sp = self.mem.space(st);
                if !sp.is_init(abs, info.size) {
                    return Err(self.uninit(b, field, t, loc));
                }
                let v = sp.read(abs, info.size);
                let id =
                    self.mp_freeze(b, t, v).map_err(|_| Freeze::Fault(FailKind::NotConstant))?;
                s.relocs.push(Reloc { at: at as u32, value: id });
                s.cover[at..at + info.size as usize].fill(1);
            }
            TyKind::Array { elem, n } => {
                let e = *self.types.info(elem);
                let es = e.size as usize;
                if matches!(e.kind, TyKind::Int { .. } | TyKind::Bool) {
                    let len = e.size * n;
                    let sp = self.mem.space(st);
                    if sp.is_init(abs, len) {
                        let (a0, a1) = (abs as usize, (abs + len) as usize);
                        s.bytes[at..at + len as usize].copy_from_slice(&sp.bytes[a0..a1]);
                        s.cover[at..at + len as usize].fill(1);
                        return Ok(());
                    }
                }
                for i in 0..n as usize {
                    let r = Ptr::with_off(raw, u64::from(p.off) + (i * es) as u64, false);
                    self.walk(b, s, elem, r, at + i * es, field, loc)?;
                }
            }
            TyKind::Record { union, first, count, .. } => {
                let (first, count) = (first as usize, count as usize);
                if union {
                    let key = Ptr::union_key(raw, u64::from(p.off));
                    let Some(&i) = self.mem.unions.get(&key) else { return Ok(()) };
                    let f = self.types.fields[first + i as usize];
                    s.unions.push(UnionMember { at: at as u32, field: f.decl });
                    return self.walk_field(b, s, raw, at, f, loc);
                }
                for i in first..first + count {
                    let f = self.types.fields[i];
                    self.walk_field(b, s, raw, at, f, loc)?;
                }
            }
            TyKind::Void | TyKind::Func | TyKind::Opaque => {
                if info.size != 0 {
                    return Err(Freeze::Fault(FailKind::NotConstant));
                }
            }
        }
        Ok(())
    }

    fn walk_field(
        &mut self,
        b: &mut dyn SemaBridge,
        s: &mut Scratch,
        raw: u64,
        at: usize,
        f: crate::ty::FieldInfo,
        loc: Loc,
    ) -> Result<(), Freeze> {
        let p = Ptr::decode(raw);
        let fr = Ptr::with_off(raw, u64::from(p.off) + f.offset, false);
        let Some(width) = f.width else {
            // clang notes an uninitialized base by its type, a member by
            // its name.
            let named = (!f.base).then_some(f.decl);
            return self.walk(b, s, f.ty, fr, at + f.offset as usize, named, loc);
        };
        if width == 0 || b.ast().decls.get(f.decl).name().is_empty() {
            return Ok(());
        }
        let n = u64::from((f.bit + width).div_ceil(8));
        let Some(a) = self.alloc_any(p).copied() else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        let abs = u64::from(a.base) + u64::from(p.off) + f.offset;
        let sp = self.mem.space(p.space());
        if !sp.is_init(abs, n) {
            return Err(self.uninit(b, Some(f.decl), f.ty, loc));
        }
        let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 } << f.bit;
        let v = sp.read(abs, n) & mask;
        let o = at + f.offset as usize;
        for i in 0..n as usize {
            s.bytes[o + i] |= (v >> (8 * i)) as u8;
            s.cover[o + i] = 1;
        }
        Ok(())
    }

    // ---- globals ------------------------------------------------------------

    /// Resolves global `g` (first use): returns its address. `top` is the
    /// first free register (for a nested evaluation of its initializer).
    pub(crate) fn resolve_global(&mut self, b: &mut dyn SemaBridge, g: u32, top: u32) -> u64 {
        let p = match self.globals[g as usize].kind {
            GKind::Str(s, ty, e) => self.alloc_string(b, s, ty, e),
            GKind::Var(d) => {
                let p = self.resolve_var(b, g, d, top);
                // A variable with no initializer yet (`extern const int
                // n;` before its definition) may get one later in the
                // translation unit: its unknown value is kept for this
                // evaluation only.
                if var_info(b.ast(), d).init.is_none() {
                    self.provisional.push(g);
                }
                p
            }
        };
        if let GState::Unresolved = self.globals[g as usize].state {
            self.globals[g as usize].state = GState::Resolved(p);
        }
        p
    }

    fn alloc_string(
        &mut self,
        b: &mut dyn SemaBridge,
        s: StringLitId,
        ty: OTy,
        e: Option<ExprId>,
    ) -> u64 {
        let size = self.types.size(ty);
        let p = self.mem.alloc_static(ty, size, flags::CONST, Origin::Str(s, e));
        let base = u64::from(
            self.mem.statics[(Ptr::decode(p).slot & !crate::mem::STATIC_BIT) as usize].base,
        );
        let bytes = b.ast().strings.bytes(s);
        let n = bytes.len().min(size as usize);
        let (a0, a1) = (base as usize, (base + size) as usize);
        self.mem.stat.bytes[a0..a1].fill(0);
        self.mem.stat.bytes[a0..a0 + n].copy_from_slice(&bytes[..n]);
        self.mem.stat.set_init(base, size, true);
        p
    }

    /// The static object of lifetime-extended temporary `t`, thawed from
    /// its `LetData.value`: readable if it is usable in constant
    /// expressions (a `const` temporary extended by a variable that is,
    /// [expr.const]/4), else an external object whose accesses clang
    /// reports as outside the expression that created it.
    fn resolve_let(&mut self, b: &mut dyn SemaBridge, t: DeclId, top: u32) -> u64 {
        if let Some(&p) = self.lets.get(&t) {
            return p;
        }
        let ast = b.ast();
        let q = ast.decls.get(t).ty().unwrap_or(QualType::INT);
        let l = ast.decls.lifetime_extended(t).copied();
        let usable = ast.types.canonical(q).has_const_bit()
            && l.is_some_and(|l| var_info(ast, l.extending).usable);
        let value = l.and_then(|l| l.value);
        let ty = self.types.intern(b, q);
        let size = self.types.size(ty);
        let hu = if self.types.info(ty).has_union { flags::HAS_UNION } else { 0 };
        let p = match value {
            Some(cv) if usable => {
                let p = self.mem.alloc_static(ty, size, flags::CONST | hu, Origin::Let(t));
                self.lets.insert(t, p);
                self.thaw_into(b, cv, p, top);
                p
            }
            _ => self.mem.alloc_static(ty, size, flags::EXTERNAL | flags::CONST, Origin::Let(t)),
        };
        self.lets.insert(t, p);
        p
    }

    fn external(&mut self, ty: OTy, d: DeclId) -> u64 {
        let size = self.types.size(ty);
        self.mem.alloc_static(ty, size, flags::EXTERNAL | flags::CONST, Origin::External(d))
    }

    fn resolve_var(&mut self, b: &mut dyn SemaBridge, g: u32, d: DeclId, top: u32) -> u64 {
        let info = var_info(b.ast(), d);
        let ty = self.types.intern(b, info.ty);
        if !info.usable {
            return self.external(ty, d);
        }
        let hu = if self.types.info(ty).has_union { flags::HAS_UNION } else { 0 };
        match info.state {
            VarEvalState::Value(cv) => {
                let size = self.types.size(ty);
                let p = self.mem.alloc_static(ty, size, flags::CONST | hu, Origin::Global(d));
                self.globals[g as usize].state = GState::Resolved(p);
                self.thaw_into(b, cv, p, top);
                p
            }
            VarEvalState::NotConstant => self.external(ty, d),
            VarEvalState::Evaluating => {
                let size = self.types.size(ty);
                self.mem.alloc_static(
                    ty,
                    size,
                    flags::CONST | flags::RESOLVING | hu,
                    Origin::Global(d),
                )
            }
            VarEvalState::NotEvaluated => {
                let Some(init) = info.init else { return self.external(ty, d) };
                if self.run.nesting >= MAX_NESTING {
                    return self.external(ty, d);
                }
                let p = self.placeholder(g, ty, d);
                self.globals[g as usize].state = GState::Resolved(p);
                let saved = (
                    self.run.counters,
                    self.mem.allocated,
                    self.mem.peak,
                    self.run.object,
                    self.run.silent,
                    self.run.const_context,
                );
                let mark = self.mem.mark();
                // Its own evaluation context: its allocations are numbered
                // from 0, leak-checked and freed at its end.
                let heap = (self.mem.heap_allocs, self.mem.ctx);
                let ctx = self.ctx_depth;
                self.ctx_depth += 1;
                self.mem.ctx = ctx;
                self.mem.heap_allocs = 0;
                self.run.nesting += 1;
                self.run.counters = Counters::default();
                self.run.object = Some(d);
                self.run.silent = true;
                // Constant initialization is manifestly constant-evaluated.
                self.run.const_context = true;
                let r = self.eval_at(b, init, Some(d), top);
                self.mem.free_heap_of(ctx, &self.types);
                self.mem.release(mark, &self.types);
                self.ctx_depth -= 1;
                (self.mem.heap_allocs, self.mem.ctx) = heap;
                self.run.nesting -= 1;
                (
                    self.run.counters,
                    self.mem.allocated,
                    self.mem.peak,
                    self.run.object,
                    self.run.silent,
                    self.run.const_context,
                ) = saved;
                // The placeholder's storage now belongs to the variable.
                self.globals[g as usize].slot = 0;
                let a = self.mem.alloc_of_mut(Ptr::decode(p));
                if let Ok(cv) = r {
                    a.flags &= !flags::RESOLVING;
                    self.thaw_into(b, cv, p, top);
                } else {
                    a.flags = flags::EXTERNAL | flags::CONST;
                    a.origin = Origin::External(d);
                }
                p
            }
        }
    }

    // ---- thawing ------------------------------------------------------------

    /// The static object holding stored constant `cv`.
    fn thaw(&mut self, b: &mut dyn SemaBridge, cv: ConstValueId, top: u32) -> u64 {
        if let Some(&p) = self.thawed.get(&cv) {
            return p;
        }
        let q = b.consts().ty(cv);
        let ty = self.types.intern(b, q);
        let size = self.types.size(ty);
        let hu = if self.types.info(ty).has_union { flags::HAS_UNION } else { 0 };
        let p = self.mem.alloc_static(ty, size, flags::CONST | hu, Origin::Const(cv));
        self.thawed.insert(cv, p);
        self.thaw_into(b, cv, p, top);
        p
    }

    /// Writes stored constant `cv` into the static object at `p`.
    pub(crate) fn thaw_into(&mut self, b: &mut dyn SemaBridge, cv: ConstValueId, p: u64, top: u32) {
        let pp = Ptr::decode(p);
        let base = u64::from(self.mem.statics[(pp.slot & !crate::mem::STATIC_BIT) as usize].base)
            + u64::from(pp.off);
        let aty = self.mem.statics[(pp.slot & !crate::mem::STATIC_BIT) as usize].ty;
        let scalar = match b.consts().get(cv) {
            ConstValue::Scalar(s) => Some(s.ty()),
            ConstValue::Aggregate(_) => None,
        };
        if scalar.is_some() {
            self.thaw_scalar(b, cv, aty, base, top);
            return;
        }
        let (relocs, unions): (Vec<Reloc>, Vec<UnionMember>) = {
            let st = b.consts();
            let Some(img) = st.aggregate(cv) else { return };
            let len = img.bytes.len() as u64;
            let a0 = base as usize;
            self.mem.stat.bytes[a0..a0 + img.bytes.len()].copy_from_slice(img.bytes);
            self.mem.stat.set_init(base, len, true);
            self.mem.stat.set_ptr(base, len, false);
            for r in img.padding {
                self.mem.stat.set_init(base + u64::from(r.start), u64::from(r.len), false);
            }
            (img.relocs.to_vec(), img.unions.to_vec())
        };
        for r in relocs {
            let at = u64::from(r.at);
            let ty = self.deepest_at(aty, at);
            if let TyKind::MemPtr { .. } = self.types.kind(ty) {
                let v = self.mp_thaw(b, r.value);
                let n = self.types.size(ty);
                self.mem.stat.write(base + at, n, v);
                continue;
            }
            let raw = self.thaw_ptr(b, r.value, ty, top);
            self.mem.stat.write(base + at, 8, u128::from(raw));
            self.mem.stat.set_ptr(base + at, 8, raw != 0);
        }
        for u in unions {
            let at = u64::from(u.at);
            let idx =
                self.types.unions(aty).iter().filter(|&&(o, _)| o == at).find_map(|&(_, ut)| {
                    self.types.fields(ut).iter().position(|f| f.decl == u.field)
                });
            if let Some(i) = idx {
                self.mem.unions.insert(Ptr::union_key(p, u64::from(pp.off) + at), i as u32);
            }
        }
    }

    fn thaw_scalar(
        &mut self,
        b: &mut dyn SemaBridge,
        cv: ConstValueId,
        ty: OTy,
        base: u64,
        top: u32,
    ) {
        let info = *self.types.info(ty);
        let len = value_len(info.size, info.kind);
        let (v, is_ptr) = match b.consts().scalar(cv) {
            Some(Scalar::Int { bits, .. }) => (bits.to_u128().unwrap_or(0), false),
            Some(Scalar::Bool { value, .. }) => (u128::from(value), false),
            Some(Scalar::Float { bits, .. }) => (bits, false),
            Some(Scalar::Pointer { .. }) => (0, true),
            Some(Scalar::MemberPtr { .. }) => (self.mp_thaw(b, cv), false),
            _ => (0, false),
        };
        let v = if is_ptr { u128::from(self.thaw_ptr(b, cv, ty, top)) } else { v };
        self.mem.stat.write(base, len, v);
        self.mem.stat.set_init(base, len, true);
        self.mem.stat.set_ptr(base, len, is_ptr && v != 0);
    }

    /// The VM pointer for stored pointer value `cv` of type `ty`.
    pub(crate) fn thaw_ptr(
        &mut self,
        b: &mut dyn SemaBridge,
        cv: ConstValueId,
        ty: OTy,
        top: u32,
    ) -> u64 {
        let Some(Scalar::Pointer { base, offset, .. }) = b.consts().scalar(cv) else { return 0 };
        let pointee = match self.types.kind(ty) {
            TyKind::Ptr(p) => Some(p),
            _ => None,
        };
        let root = match base {
            PtrBase::Function(d) => {
                let f = self.func_index(b.ast(), d);
                return fn_ptr(f);
            }
            PtrBase::Decl(d) => {
                let g = self.global_index(b.ast(), d);
                match self.globals[g as usize].state {
                    GState::Resolved(p) => p,
                    GState::Unresolved => self.resolve_global(b, g, top),
                }
            }
            // The object of the literal's own evaluation: a pointer frozen
            // from `"a"` thaws to the object `"a"` creates, so it compares
            // equal to itself and unspecified to other literals.
            PtrBase::StringLit { lit, origin } => {
                let ast = b.ast();
                let (enc, units) = (ast.strings.encoding(lit), ast.strings.unit_count(lit));
                let cq = match enc {
                    StringEncoding::Ordinary => QualType::CHAR,
                    StringEncoding::Utf8 => QualType::CHAR8,
                    StringEncoding::Utf16 => QualType::CHAR16,
                    StringEncoding::Utf32 => QualType::CHAR32,
                    StringEncoding::Wide16 | StringEncoding::Wide32 => QualType::WCHAR,
                };
                let g = match self.lit_exprs.get(&origin) {
                    Some(&g) => g,
                    None => {
                        let et = self.types.intern(b, cq);
                        let at = self.types.array_of(et, units + 1);
                        self.string_expr_index(origin, lit, at)
                    }
                };
                match self.globals[g as usize].state {
                    GState::Resolved(p) => p,
                    GState::Unresolved => self.resolve_global(b, g, top),
                }
            }
            PtrBase::Const(c) => self.thaw(b, c, top),
            PtrBase::Temporary(d) => self.resolve_let(b, d, top),
            PtrBase::TypeInfo(_) => {
                let t = pointee.unwrap_or(ty);
                let size = self.types.size(t);
                self.mem.alloc_static(t, size, flags::CONST, Origin::None)
            }
        };
        let rp = Ptr::decode(root);
        let Some(a) = self.alloc_any(rp).copied() else { return root };
        let past = match pointee {
            Some(pt) if !matches!(self.types.kind(pt), TyKind::Void | TyKind::Opaque) => {
                offset >= u64::from(a.size) || !self.types.has_subobject(a.ty, offset, pt)
            }
            _ => offset >= u64::from(a.size),
        };
        Ptr::with_off(root, offset, past)
    }
}

fn ast_first(b: &dyn SemaBridge, d: DeclId) -> DeclId {
    b.ast().first(d)
}

/// The `MaterializeTemporaryExpr` that created static temporary `t` (in
/// its extending variable's initializer), for "temporary created here".
pub(crate) fn let_mte(ast: &Ast, t: DeclId) -> Option<ExprId> {
    let ext = ast.decls.lifetime_extended(t)?.extending;
    let init = ast.redecls_of(ext).find_map(|r| ast.decls.var(r).and_then(|v| v.init))?;
    let mut work = vec![init];
    while let Some(e) = work.pop() {
        if ast.exprs.get(e).kind() == kiln_ast::expr::ExprKind::MaterializeTemporaryExpr
            && ast.exprs.lifetime_extended_temporary(e) == Some(t)
        {
            return Some(e);
        }
        work.extend(ast.exprs.children(e, &ast.lists));
    }
    None
}
