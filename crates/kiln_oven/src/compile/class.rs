//! Lowering of class semantics (WP-076; spec/oven.md "Classes"): the
//! implicit object parameter, constructors and their ctor-initializers,
//! destructors (registered with `Op::PushCleanup`, run in reverse by the
//! enclosing `ScopeExit`/`Ret`), temporaries and their materialization,
//! member calls, and derived-to-base / base-to-derived conversions.
//!
//! Base subobjects are laid out by `ty.rs` as leading `FieldInfo`s with
//! `base: true` (empty bases are omitted, at offset 0); a conversion along
//! a base path is an offset (`Op::Field`), checked in the other direction
//! by `Op::Downcast`.

use kiln_ast::decl::{CtorInit, CtorInitKind, DeclKind, DefinitionBits, FnStorage};
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, StorageDuration, UnaryOp};
use kiln_ast::{Ast, DeclId, ExprId, QualType, TypeId};
use kiln_source::Loc;

use super::new_delete::record_bits;
use super::{Access, Compiler, Local, Place, skip_parens};
use crate::kbc::{Op, cmp, int_aux};
use crate::mem::Origin;
use crate::ty::{OTy, TyKind};

/// A member call through a null object pointer.
const NULL_MEMBER_CALL: &str =
    "member call on dereferenced null pointer is not allowed in a constant expression";

/// What a call calls.
#[derive(Clone, Copy)]
pub(super) enum Callee {
    Direct(DeclId),
    /// A function pointer in a register.
    Ind(u32),
}

/// Whether function `d` has an implicit object parameter (`this`): a
/// non-static member function without an explicit object parameter, or a
/// constructor or destructor.
pub(super) fn has_this(ast: &Ast, d: DeclId) -> bool {
    matches!(
        ast.decls.get(d).kind(),
        DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion
    ) && ast
        .decls
        .function(d)
        .is_some_and(|f| f.spec.storage() != FnStorage::Static && !f.spec.has_explicit_object())
}

/// The note of a construct needing virtual dispatch or virtual bases.
const VIRTUAL_UNSUPPORTED: &str =
    "kiln: virtual dispatch and virtual base classes in constant evaluation land with WP-077";

impl Compiler<'_> {
    /// The current `this`: the object a default member initializer is
    /// initializing, else the implicit object parameter.
    fn this_ptr(&self) -> Option<u32> {
        self.this_override.or(self.this_reg)
    }

    /// `this` into `dst` ([expr.prim.this]).
    pub(super) fn this_expr(&mut self, e: ExprId, dst: u32) {
        match self.this_ptr() {
            Some(t) => {
                if t != dst {
                    self.emit(Op::Mov, 0, &[dst, t]);
                }
            }
            None => self.trap(
                e,
                "use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function",
            ),
        }
    }

    /// The byte offset of the direct, non-virtual base `base` in class
    /// `cls` (an empty base, which is not laid out, is at 0).
    pub(super) fn base_offset(&self, cls: OTy, base: OTy) -> u64 {
        self.o.types.fields(cls).iter().find(|f| f.base && f.ty == base).map_or(0, |f| f.offset)
    }

    /// The offset of the base subobject reached from class `from` along
    /// cast `e`'s base path, and the base's type; `None` through a virtual
    /// base.
    fn path_offset(&mut self, e: ExprId, from: OTy) -> Option<(u64, OTy)> {
        let ast = self.ast();
        let Some(path) = ast.exprs.cast_path(e) else { return Some((0, from)) };
        let specs: Vec<(QualType, bool)> = ast
            .lists
            .get(path)
            .iter()
            .map(|&i| {
                let b = ast.decls.base_spec(i);
                (b.ty, b.is_virtual)
            })
            .collect();
        let mut off = 0;
        let mut cur = from;
        for (q, virt) in specs {
            if virt {
                return None;
            }
            let b = self.ty(q);
            off += self.base_offset(cur, b);
            cur = b;
        }
        Some((off, cur))
    }

    /// A derived-to-base (`down` false) or base-to-derived conversion of
    /// the object address `p` (a pointer: null stays null) into `dst`.
    /// `ty` is the result's (pointee) type.
    pub(super) fn class_cast(&mut self, e: ExprId, p: u32, dst: u32, ptr: bool, down: bool) {
        let ast = self.ast();
        let op = ast.exprs.operand(e);
        let pointee = |q: QualType| ast.types.pointee(q).filter(|_| ptr).unwrap_or(q);
        let (oq, rq) = (pointee(ast.exprs.get(op).ty()), pointee(ast.exprs.get(e).ty()));
        let derived = self.ty(if down { rq } else { oq });
        let Some((off, _)) = self.path_offset(e, derived) else {
            self.trap(e, VIRTUAL_UNSUPPORTED);
            return;
        };
        if down {
            self.emit_at(Op::Downcast, u32::from(ptr), &[dst, p, off as u32, derived.0], e);
        } else if off == 0 {
            if p != dst {
                self.emit(Op::Mov, 0, &[dst, p]);
            }
        } else {
            self.emit_at(Op::Field, u32::from(ptr), &[dst, p, off as u32], e);
        }
    }

    // ---- destruction ------------------------------------------------------

    /// The function index of the destructor run for an object of type
    /// `ty` (each element of an array), if it is not trivial.
    pub(super) fn dtor_of(&mut self, ty: OTy) -> Option<u32> {
        let mut t = ty;
        while let TyKind::Array { elem, .. } = self.kind(t) {
            t = elem;
        }
        let TyKind::Record { decl, .. } = self.kind(t) else { return None };
        let ast = self.b.ast();
        if record_bits(ast, decl).contains(DefinitionBits::TRIVIAL_DTOR) {
            return None;
        }
        let d = ast.destructor(decl)?;
        Some(self.o.func_index(self.b.ast(), d))
    }

    /// Registers the destruction of the object of type `ty` at `p` with
    /// the enclosing scope (aux 1: `p` is a heap array of `ty`s).
    pub(super) fn push_cleanup(&mut self, p: u32, ty: OTy, aux: u32, loc: Loc) {
        if let Some(f) = self.dtor_of(ty) {
            self.emit_loc(Op::PushCleanup, aux, &[p, f, ty.0], loc);
        }
    }

    /// Materializes prvalue `e` of type `ty` as a temporary destroyed at
    /// the end of the enclosing full-expression ([class.temporary]/4);
    /// returns the register holding its address.
    pub(super) fn materialize(&mut self, e: ExprId, ty: OTy) -> u32 {
        self.materialize_as(e, ty, false)
    }

    /// `materialize`, the temporary const when `is_const` (a class
    /// object only once constructed, [class.ctor.general]/5: its const
    /// type is left to its destructor's `this`).
    fn materialize_as(&mut self, e: ExprId, ty: OTy, is_const: bool) -> u32 {
        let is_const = is_const && self.is_late_const(ty).is_none();
        let r = self.reg();
        self.alloc(r, ty, Origin::Temp(e), is_const, e);
        self.init(e, r, ty);
        let loc = self.begin(e);
        self.push_cleanup(r, ty, 0, loc);
        r
    }

    /// `MaterializeTemporaryExpr` `e` as a glvalue.
    pub(super) fn mte(&mut self, e: ExprId) -> Place {
        let ast = self.ast();
        let x = ast.exprs.operand(e);
        let storage = ast.exprs.temporary_storage(e);
        let ty = self.ety(x);
        if let Some(&r) = self.ext_temps.get(&e) {
            // Lifetime-extended by the variable being initialized: its
            // storage was allocated, and its destruction is registered,
            // outside the initializer's full-expression.
            self.init(x, r, ty);
            return Place::Mem(r);
        }
        match storage {
            StorageDuration::FullExpression => {
                let is_const = super::const_object(self.ast(), self.ast().exprs.get(e).ty());
                Place::Mem(self.materialize_as(x, ty, is_const))
            }
            // Extended by a variable outside this evaluation, or by one
            // whose storage was not preallocated: lives until the
            // evaluation ends.
            _ => {
                let r = self.reg();
                self.alloc(r, ty, Origin::Temp(x), false, x);
                self.init(x, r, ty);
                Place::Mem(r)
            }
        }
    }

    /// The `MaterializeTemporaryExpr`s whose temporaries initializer
    /// `init` of a local variable extends ([class.temporary]/6), with
    /// their types.
    fn extended_temps(&mut self, init: ExprId, sd: StorageDuration) -> Vec<(ExprId, ExprId)> {
        let ast = self.ast();
        let mut out = Vec::new();
        let mut work = vec![init];
        while let Some(e) = work.pop() {
            let n = ast.exprs.get(e);
            match n.kind() {
                ExprKind::MaterializeTemporaryExpr => {
                    if ast.exprs.temporary_storage(e) == sd {
                        out.push((e, ast.exprs.operand(e)));
                    }
                    work.push(ast.exprs.operand(e));
                }
                // Not part of this initializer's evaluation.
                ExprKind::LambdaExpr | ExprKind::StmtExpr => {}
                _ => work.extend(ast.exprs.children(e, &ast.lists)),
            }
        }
        out.reverse();
        out
    }

    /// Allocates the storage of the temporaries initializer `init` of
    /// local `d` extends; returns them for [`Self::end_extended`].
    pub(super) fn begin_extended(&mut self, init: ExprId) -> Vec<(u32, OTy, Loc)> {
        if !self.has_temps {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (mte, x) in self.extended_temps(init, StorageDuration::Automatic) {
            let ty = self.ety(x);
            let r = self.reg();
            self.alloc(r, ty, Origin::Temp(x), false, x);
            self.ext_temps.insert(mte, r);
            let loc = self.begin(x);
            out.push((r, ty, loc));
        }
        out
    }

    /// Allocates the storage of the temporaries initializer `init` of
    /// variable `d` (static storage duration) extends
    /// ([class.temporary]/6): objects of their own, outliving the
    /// initializer's full-expression, which `d`'s constant value may
    /// point to (frozen into their `LetData.value`s).
    pub(super) fn begin_static_extended(&mut self, init: ExprId, d: DeclId) {
        let temps = self.extended_temps(init, StorageDuration::Static);
        for (mte, x) in temps {
            let ast = self.ast();
            let Some(t) = ast.exprs.lifetime_extended_temporary(mte) else { continue };
            let extends = ast
                .decls
                .lifetime_extended(t)
                .is_some_and(|l| ast.first(l.extending) == ast.first(d));
            if !extends {
                continue;
            }
            let ty = self.ety(x);
            let r = self.reg();
            self.alloc(r, ty, Origin::Let(t), false, x);
            self.ext_temps.insert(mte, r);
        }
    }

    /// Registers the destruction of the extended temporaries (after the
    /// variable's initialization; they are destroyed with it).
    pub(super) fn end_extended(&mut self, temps: &[(u32, OTy, Loc)]) {
        for &(r, ty, loc) in temps {
            self.push_cleanup(r, ty, 0, loc);
        }
    }

    /// After the initialization of local `d` at `p`: registers its
    /// destruction, and makes a `const` class object const.
    pub(super) fn end_local_init(&mut self, d: DeclId, p: u32, ty: OTy) {
        let loc = self.b.ast().decls.get(d).loc();
        self.push_cleanup(p, ty, 0, loc);
        if self.late_const.contains(&d) {
            self.emit_loc(Op::MakeConst, 0, &[p], loc);
        }
    }

    /// Whether a `const` local of type `ty` is made const only once
    /// initialized (a class object, [class.ctor.general]/5): the class
    /// has no mutable members (which stay modifiable; kiln has no
    /// per-member constness, so the object stays modifiable).
    pub(super) fn is_late_const(&mut self, ty: OTy) -> Option<bool> {
        let mut t = ty;
        while let TyKind::Array { elem, .. } = self.kind(t) {
            t = elem;
        }
        let TyKind::Record { decl, .. } = self.kind(t) else { return None };
        Some(!record_bits(self.b.ast(), decl).contains(DefinitionBits::HAS_MUTABLE_FIELDS))
    }

    // ---- construction -----------------------------------------------------

    /// The scan of the ctor-initializers of constructor `def`
    /// (temporaries, locals whose address is taken).
    pub(super) fn scan_inits(&mut self, def: DeclId) {
        let ast = self.b.ast();
        let inits: Vec<ExprId> = ast.ctor_inits(def).map(|c| c.init).collect();
        let mut s = super::Scan { ast, mem: &mut self.mem_vars, cur: None, temps: false };
        for e in inits {
            s.expr(e, false);
        }
        self.has_temps |= s.temps;
    }

    /// Before the body of member function `def`: a constructor's
    /// ctor-initializers in order ([class.base.init]/13); a destructor's
    /// registration of the destruction of its members and bases, run in
    /// reverse after its body ([class.dtor]/13).
    pub(super) fn member_prologue(&mut self, def: DeclId) {
        let Some(this) = self.this_reg else { return };
        let ast = self.ast();
        let kind = ast.decls.get(def).kind();
        if !matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXDestructor) {
            return;
        }
        let Some(cq) = ast.decls.get(def).ctx().and_then(|c| ast.decls.get(c).ty()) else {
            return;
        };
        let cls = self.ty(cq);
        let TyKind::Record { union, decl: rdef, .. } = self.kind(cls) else { return };
        if kind == DeclKind::CXXConstructor {
            let inits: Vec<CtorInit> = self.ast().ctor_inits(def).copied().collect();
            for ci in inits {
                let m = self.next;
                self.ctor_init(&ci, this, cls, union);
                self.next = m;
            }
            return;
        }
        if union {
            // A union's destructor destroys no member.
            return;
        }
        let loc = self.ast().decls.get(def).loc();
        let ast = self.ast();
        let bases: Vec<(QualType, bool)> =
            ast.decls.bases(rdef, &ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
        for (q, virt) in bases {
            if virt {
                continue;
            }
            let bty = self.ty(q);
            let off = self.base_offset(cls, bty);
            let m = self.next;
            let q = self.elem_ptr_loc(this, off, loc);
            self.push_cleanup(q, bty, 0, loc);
            self.next = m;
        }
        let fields = self.o.types.fields(cls).to_vec();
        for fi in fields {
            if fi.base || fi.width.is_some() || super::is_reference_decl(self.ast(), fi.decl) {
                continue;
            }
            if self.dtor_of(fi.ty).is_none() {
                continue;
            }
            let m = self.next;
            let q = self.elem_ptr_loc(this, fi.offset, loc);
            self.push_cleanup(q, fi.ty, 0, loc);
            self.next = m;
        }
    }

    /// `p + off` (bytes), the site at `loc`.
    fn elem_ptr_loc(&mut self, p: u32, off: u64, loc: Loc) -> u32 {
        if off == 0 {
            return p;
        }
        let r = self.reg();
        self.emit_loc(Op::Field, 0, &[r, p, off as u32], loc);
        r
    }

    fn ctor_init(&mut self, ci: &CtorInit, this: u32, cls: OTy, union: bool) {
        match ci.kind {
            CtorInitKind::Base => {
                let bty = self.ty(QualType::unqual(TypeId::new(ci.target)));
                let off = self.base_offset(cls, bty);
                let q = self.elem_ptr(this, off, ci.init);
                self.init(ci.init, q, bty);
            }
            CtorInitKind::VirtualBase => self.trap_loc(ci.loc, VIRTUAL_UNSUPPORTED.into()),
            CtorInitKind::Field => {
                let fd = DeclId::new(ci.target);
                let fields = self.o.types.fields(cls);
                let Some(idx) = fields.iter().position(|f| !f.base && f.decl == fd) else {
                    self.unsupported(ci.init);
                    return;
                };
                let fi = fields[idx];
                if union {
                    self.emit_at(Op::SetActive, 1, &[this, idx as u32, cls.0], ci.init);
                }
                self.init_field(ci.init, this, fi, ci.init);
            }
            CtorInitKind::IndirectField => {
                let ast = self.ast();
                let chain: Vec<DeclId> = ast
                    .lists
                    .get(ast.indirect_field_chain(DeclId::new(ci.target)))
                    .iter()
                    .map(|&d| DeclId::new(d))
                    .collect();
                let (mut obj, mut oty) = (this, cls);
                for (k, fd) in chain.iter().enumerate() {
                    let fields = self.o.types.fields(oty);
                    let Some(idx) = fields.iter().position(|f| !f.base && f.decl == *fd) else {
                        self.unsupported(ci.init);
                        return;
                    };
                    let fi = fields[idx];
                    if matches!(self.kind(oty), TyKind::Record { union: true, .. }) {
                        self.emit_at(Op::SetActive, 1, &[obj, idx as u32, oty.0], ci.init);
                    }
                    if k + 1 == chain.len() {
                        self.init_field(ci.init, obj, fi, ci.init);
                    } else {
                        obj = self.elem_ptr(obj, fi.offset, ci.init);
                        oty = fi.ty;
                    }
                }
            }
            CtorInitKind::Delegating => self.init(ci.init, this, cls),
        }
    }

    /// Which parameters of function `d` are references.
    fn proto_refs(&self, d: DeclId) -> Vec<bool> {
        let ast = self.ast();
        ast.decls
            .get(d)
            .ty()
            .and_then(|q| ast.types.proto(q))
            .map(|p| ast.types.params(p).iter().map(|&t| ast.types.is_reference(t)).collect())
            .unwrap_or_default()
    }

    /// `CXXConstructExpr` / `CXXTemporaryObjectExpr` `e` initializing the
    /// object (or array of objects) of type `ty` at `p`.
    pub(super) fn construct(&mut self, e: ExprId, p: u32, ty: OTy) {
        let ast = self.ast();
        let zeroing = ast.exprs.construct_info(e).zeroing;
        let trivial = ast.is_trivial_default_construction(e);
        if zeroing {
            // [dcl.init.general]/9.1.2: zero-initialized first.
            self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
        }
        if trivial {
            return;
        }
        self.construct_obj(e, p, ty);
    }

    fn construct_obj(&mut self, e: ExprId, p: u32, ty: OTy) {
        if let TyKind::Array { elem, n } = self.kind(ty) {
            // Each element in order ([dcl.init.general]/7.2).
            let m = self.next;
            let k = self.reg();
            let one = self.reg();
            let lim = self.reg();
            let c = self.reg();
            self.konst(k, 0);
            self.konst(one, 1);
            self.konst(lim, u128::from(n));
            let top = self.here();
            self.emit(Op::ICmp, int_aux(64, false) | cmp::LT << 9, &[c, k, lim]);
            let exit = self.jmp_if(c, false);
            let q = self.reg();
            self.emit_at(Op::PtrAdd, int_aux(64, false), &[q, p, k, elem.0], e);
            self.construct_obj(e, q, elem);
            self.emit(Op::IAdd, int_aux(64, false), &[k, k, one]);
            let j = self.jmp();
            self.patch(j, top);
            self.patch_here(exit);
            self.next = m;
            return;
        }
        let ast = self.ast();
        let ctor = ast.exprs.constructor(e);
        let args: Vec<ExprId> = ast.lists.iter::<ExprId>(ast.exprs.construct_args(e)).collect();
        let trivial = ast.decls.function(ctor).is_some_and(|f| f.spec.is_trivial());
        if trivial {
            match args[..] {
                [] => {}
                // A trivial copy/move constructor copies the object
                // representation ([class.copy.ctor]/14).
                [a] => {
                    let a = skip_parens(self.ast(), a);
                    if self.node(a).value_category().is_glvalue() {
                        let src = self.lv_mem(a, Access::Read);
                        self.emit_at(Op::Copy, 1, &[p, src, ty.0], e);
                    } else {
                        self.init_inner(a, p, ty);
                    }
                }
                _ => self.unsupported(e),
            }
            return;
        }
        let refs = self.proto_refs(ctor);
        let void = self.ty(QualType::VOID);
        let loc = self.ctor_loc(e);
        self.call_frame(e, Callee::Direct(ctor), void, None, Some(p), &args, &refs, Some(loc));
    }

    /// Where clang reports a constructor call (the call-stack note).
    fn ctor_loc(&self, e: ExprId) -> Loc {
        self.ast().exprs.get(e).loc()
    }

    /// `CXXInheritedCtorInitExpr` `e` (in an inheriting constructor):
    /// the base constructor called with the constructor's own arguments
    /// ([class.inhctor.init]/1).
    pub(super) fn inherited_ctor(&mut self, e: ExprId, p: u32) {
        let ast = self.ast();
        let ctor = ast.exprs.constructor(e);
        if ast.exprs.inherited_ctor_info(e).1 {
            self.trap(e, VIRTUAL_UNSUPPORTED);
            return;
        }
        let params = self.cur_params.clone();
        let refs = self.proto_refs(ctor);
        let base = self.next;
        let window = params.len() as u32 + 1;
        for _ in 0..window {
            self.reg();
        }
        self.emit(Op::Mov, 0, &[base, p]);
        for (i, &pd) in params.iter().enumerate() {
            let slot = base + 1 + i as u32;
            let q = self.ast().decls.get(pd).ty().unwrap_or(QualType::INT);
            let pty = self.ty(q);
            let is_ref = refs.get(i).copied().unwrap_or(false);
            match self.locals.get(&pd).copied() {
                Some(Local::Reg(r)) => {
                    self.emit(Op::Mov, 0, &[slot, r]);
                }
                Some(Local::Mem(a)) if is_ref => {
                    self.emit(Op::Mov, 0, &[slot, a]);
                }
                Some(Local::Mem(a)) if self.o.types.is_aggregate(pty) => {
                    self.alloc(slot, pty, Origin::Temp(e), false, e);
                    self.emit_at(Op::Copy, 1, &[slot, a, pty.0], e);
                    let loc = self.ast().exprs.get(e).loc();
                    self.push_cleanup(slot, pty, 0, loc);
                }
                Some(Local::Mem(a)) => {
                    self.emit_at(Op::Load, crate::kbc::ak::READ, &[slot, a, pty.0], e);
                }
                None => self.unsupported(e),
            }
        }
        let f = self.o.func_index(self.b.ast(), ctor);
        self.emit_at(Op::Call, 0, &[f, base], e);
        self.next = base + 1;
    }

    // ---- calls ------------------------------------------------------------

    /// Emits a call (site `e`, its note at `loc` when given) of `callee`
    /// returning `rty` with the window `[sret][this][args]`: `sret` is the
    /// result object (else a temporary), `this` the object argument,
    /// `refs` which parameters are references. By-value class arguments
    /// are materialized by the caller and destroyed at the end of its
    /// full-expression. Returns the window base (the scalar result).
    #[allow(clippy::too_many_arguments, reason = "the parts of a call")]
    pub(super) fn call_frame(
        &mut self,
        e: ExprId,
        callee: Callee,
        rty: OTy,
        sret: Option<u32>,
        this: Option<u32>,
        args: &[ExprId],
        refs: &[bool],
        loc: Option<Loc>,
    ) -> u32 {
        let is_sret = self.o.types.is_aggregate(rty);
        let base = self.next;
        let first = u32::from(is_sret) + u32::from(this.is_some());
        let window = args.len() as u32 + first;
        for _ in 0..window.max(1) {
            self.reg();
        }
        if is_sret {
            match sret {
                Some(p) => {
                    self.emit(Op::Mov, 0, &[base, p]);
                }
                None => self.alloc(base, rty, Origin::Temp(e), false, e),
            }
        }
        if let Some(t) = this {
            self.emit(Op::Mov, 0, &[base + u32::from(is_sret), t]);
        }
        for (i, &a) in args.iter().enumerate() {
            let slot = base + first + i as u32;
            let aty = self.ety(a);
            if refs.get(i).copied().unwrap_or(false) {
                self.bind_ref(a, slot);
            } else if self.o.types.is_aggregate(aty) {
                self.alloc(slot, aty, Origin::Temp(a), false, a);
                self.init(a, slot, aty);
                let at = self.begin(a);
                self.push_cleanup(slot, aty, 0, at);
            } else {
                self.rv_into(a, slot);
            }
        }
        let pc = match callee {
            Callee::Direct(d) => {
                let f = self.o.func_index(self.b.ast(), d);
                self.emit_at(Op::Call, 0, &[f, base], e)
            }
            Callee::Ind(r) => self.emit_at(Op::CallInd, 0, &[r, base], e),
        };
        if let Some(loc) = loc
            && let Some(site) = self.o.sites.last_mut().filter(|x| x.pc == pc as u32)
        {
            site.loc = loc;
        }
        self.next = base + 1;
        base
    }

    /// Whether the object expression `obj` of a member call designates an
    /// object whose dynamic type is its static type (so a virtual call
    /// needs no dispatch): a variable or temporary of class type, or
    /// `*this` in a constructor or destructor ([class.cdtor]/4).
    fn exact_dynamic_type(&self, obj: ExprId, arrow: bool) -> bool {
        let ast = self.ast();
        let mut x = skip_parens(ast, obj);
        while ast.exprs.get(x).kind().is_cast() && ast.exprs.cast_kind(x) == CastKind::NoOp {
            x = skip_parens(ast, ast.exprs.operand(x));
        }
        let n = ast.exprs.get(x);
        match n.kind() {
            ExprKind::CXXThisExpr => {
                arrow
                    && self.cur_fn.is_some_and(|f| {
                        matches!(
                            ast.decls.get(f).kind(),
                            DeclKind::CXXConstructor | DeclKind::CXXDestructor
                        )
                    })
            }
            ExprKind::DeclRefExpr if !arrow => {
                let d = ast.exprs.decl(x);
                matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::ParmVar)
                    && !super::is_reference_decl(ast, d)
            }
            ExprKind::MaterializeTemporaryExpr => !arrow,
            _ => !arrow && !n.value_category().is_glvalue(),
        }
    }

    /// `CXXMemberCallExpr` `e`: the scalar result in the returned register
    /// (`sret`: the result object of a class result).
    pub(super) fn member_call(&mut self, e: ExprId, sret: Option<u32>) -> u32 {
        let ast = self.ast();
        let callee = skip_parens(ast, ast.exprs.callee(e));
        let args: Vec<ExprId> = ast.lists.iter::<ExprId>(ast.exprs.call_args(e)).collect();
        if ast.exprs.get(callee).kind() != ExprKind::MemberExpr {
            if let Some(bin) = self.is_mp_callee(callee) {
                return self.mp_call(e, bin, sret);
            }
            self.unsupported(e);
            return self.reg();
        }
        let obj = ast.exprs.member_call_object(e);
        let md = ast.exprs.decl(callee);
        let arrow = ast.exprs.get(callee).has(ExprFlags::ARROW);
        let qualified = ast.exprs.decl_ref_extra(callee).is_some_and(|x| x.qualifier.is_some());
        self.method_call(e, callee, md, obj, arrow, qualified, &args, sret)
    }

    /// A call of member function `md` on object expression `obj`
    /// (`arrow`: `obj` is a pointer) with the remaining arguments `args`:
    /// a `CXXMemberCallExpr`, or a `CXXOperatorCallExpr` whose callee is a
    /// member (its first argument the object, [over.match.oper]/2).
    #[allow(clippy::too_many_arguments, reason = "the call's parts, as decomposed by both callers")]
    pub(super) fn method_call(
        &mut self,
        e: ExprId,
        callee: ExprId,
        md: DeclId,
        obj: ExprId,
        arrow: bool,
        qualified: bool,
        args: &[ExprId],
        sret: Option<u32>,
    ) -> u32 {
        let rty = self.ety(e);
        let ast = self.ast();
        let spec = ast.decls.function(md).map(|f| f.spec);
        let class_final = ast
            .decls
            .get(md)
            .ctx()
            .is_some_and(|c| record_bits(ast, c).contains(DefinitionBits::FINAL));
        let refs = self.proto_refs(md);
        let loc = self.begin(e);
        if spec.is_some_and(|s| s.storage() == FnStorage::Static) {
            self.effect(obj);
            return self.call_frame(e, Callee::Direct(md), rty, sret, None, args, &refs, Some(loc));
        }
        if spec.is_some_and(|s| s.is_virtual() && !s.is_final())
            && !qualified
            && !class_final
            && !self.exact_dynamic_type(obj, arrow)
        {
            self.trap(e, VIRTUAL_UNSUPPORTED);
            return self.reg();
        }
        let this = if arrow {
            self.rv(obj)
        } else if self.node(obj).value_category().is_glvalue() {
            self.lv_mem(obj, Access::Addr)
        } else {
            let oty = self.ety(obj);
            self.materialize(obj, oty)
        };
        let deref = {
            let ast = self.ast();
            let o = skip_parens(ast, obj);
            ast.exprs.get(o).kind() == ExprKind::UnaryOperator
                && ast.exprs.unary_op(o) == UnaryOp::Deref
        };
        if arrow || deref {
            // [expr.ref]/8 (clang's `CSK_MemberCall` check): the object
            // expression designates no object.
            let m = self.next;
            let c = self.reg();
            self.emit(Op::PtrToBool, 0, &[c, this]);
            let ok = self.jmp_if(c, true);
            self.trap(callee, NULL_MEMBER_CALL);
            self.patch_here(ok);
            self.next = m;
        }
        self.call_frame(e, Callee::Direct(md), rty, sret, Some(this), args, &refs, Some(loc))
    }

    // ---- aggregates ---------------------------------------------------------

    /// The initializer a `CXXDefaultInitExpr` stands for: its field's
    /// default member initializer.
    pub(super) fn default_init_of(&self, e: ExprId) -> Option<ExprId> {
        let ast = self.ast();
        let f = ast.exprs.default_init_field(e);
        ast.decls.field(f).and_then(|x| x.init)
    }

    /// Whether `x` is a default member initializer use (its `this` is the
    /// object being initialized).
    pub(super) fn is_default_init(&self, x: ExprId) -> bool {
        let ast = self.ast();
        ast.exprs.get(skip_parens(ast, x)).kind() == ExprKind::CXXDefaultInitExpr
    }

    /// `ArrayInitLoopExpr` `e`: each element of the array at `p`
    /// initialized from its element initializer, the common source
    /// evaluated once ([dcl.init.general]/16.6 copies of arrays).
    pub(super) fn array_init_loop(&mut self, e: ExprId, p: u32, ty: OTy) {
        let ast = self.ast();
        let [common, elem_init] = ast.exprs.array_init_loop(e);
        let src = ast.exprs.opaque_source(common).unwrap_or(common);
        let TyKind::Array { elem, n } = self.kind(ty) else {
            self.unsupported(e);
            return;
        };
        let m = self.next;
        let v = if self.node(src).value_category().is_glvalue() {
            super::Opaque::Lv(self.lv(src, Access::Read))
        } else {
            let sty = self.ety(src);
            super::Opaque::Lv(Place::Mem(self.materialize(src, sty)))
        };
        self.opaque.insert(src, v);
        let k = self.reg();
        let one = self.reg();
        let lim = self.reg();
        let c = self.reg();
        self.konst(k, 0);
        self.konst(one, 1);
        self.konst(lim, u128::from(n));
        let saved = self.array_index.replace(k);
        let top = self.here();
        self.emit(Op::ICmp, int_aux(64, false) | cmp::LT << 9, &[c, k, lim]);
        let exit = self.jmp_if(c, false);
        let q = self.reg();
        self.emit_at(Op::PtrAdd, int_aux(64, false), &[q, p, k, elem.0], e);
        self.init(elem_init, q, elem);
        self.emit(Op::IAdd, int_aux(64, false), &[k, k, one]);
        let j = self.jmp();
        self.patch(j, top);
        self.patch_here(exit);
        self.array_index = saved;
        self.opaque.remove(&src);
        self.next = m;
    }
}
