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
use kiln_ast::expr::{CastKind, ConstructionKind, ExprFlags, ExprKind, StorageDuration, UnaryOp};
use kiln_ast::{Ast, DeclId, ExprId, QualType, TypeId};
use kiln_source::Loc;

use super::new_delete::record_bits;
use super::{Access, Compiler, ElemTrack, Local, Place, skip_parens};
use crate::kbc::{Op, cmp, int_aux};
use crate::mem::Origin;
use crate::ty::{OTy, TyKind};
use crate::vm::exc::LIFE_CHECK;

/// A member call through a null object pointer.
const NULL_MEMBER_CALL: &str =
    "member call on dereferenced null pointer is not allowed in a constant expression";

/// What a call calls.
#[derive(Clone, Copy)]
pub(super) enum Callee {
    Direct(DeclId),
    /// The base-object variant of a constructor of a class with virtual
    /// bases ([`crate::compile::Func::base_obj`]).
    Base(DeclId),
    /// A virtual call of the method (dispatched on the dynamic type of
    /// the object argument), a member of the class.
    Virtual(DeclId, OTy),
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

impl Compiler<'_> {
    /// Whether clang's `CheckLiteralType` on a local variable or by-value
    /// argument is skipped: clang 22 drops it from C++23 (P2448), where a
    /// non-literal object is only rejected through the non-constexpr
    /// function its construction calls; clang 19 still checks it.
    pub(super) fn literal_check_relaxed(&self) -> bool {
        self.o.clang_compat >= 22 && self.b.cxx23()
    }

    /// The current `this`: the object a default member initializer is
    /// initializing, else the `this` a lambda captured, else the implicit
    /// object parameter.
    fn this_ptr(&self) -> Option<u32> {
        self.this_override.or(self.lambda_this).or(self.this_reg)
    }

    /// `this` into `dst` ([expr.prim.this]).
    pub(super) fn this_expr(&mut self, e: ExprId, dst: u32) {
        match self.this_ptr() {
            Some(t) => {
                if t != dst {
                    self.emit(Op::Mov, 0, &[dst, t]);
                }
            }
            None => {
                // clang `note_constexpr_this`: "implicit use" for a member
                // named without `this->`.
                let msg = if self.b.ast().exprs.this_is_implicit(e) {
                    "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
                } else {
                    "use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
                };
                self.trap(e, msg);
            }
        }
    }

    /// The byte offset of the direct, non-virtual base `base` in class
    /// `cls` (an empty base, which is not laid out, is at 0).
    pub(super) fn base_offset(&self, cls: OTy, base: OTy) -> u64 {
        let t = &self.o.types;
        t.fields(cls)[..t.vbase_start(cls)]
            .iter()
            .find(|f| f.base && t.same_class(f.ty, base))
            .map_or(0, |f| f.offset)
    }

    /// The offset of the base subobject reached from class `from` along
    /// cast `e`'s base path up to its first virtual base, the class
    /// there, and the rest of the path (from that virtual base on).
    fn path_offset(&mut self, e: ExprId, from: OTy) -> (u64, OTy, Vec<QualType>) {
        let ast = self.ast();
        let Some(path) = ast.exprs.cast_path(e) else { return (0, from, Vec::new()) };
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
        for (i, &(q, virt)) in specs.iter().enumerate() {
            if virt {
                return (off, cur, specs[i..].iter().map(|s| s.0).collect());
            }
            let b = self.ty(q);
            off += self.base_offset(cur, b);
            cur = b;
        }
        (off, cur, Vec::new())
    }

    /// The index in the subobject graph of class `cls` of the base
    /// reached along `rest` (a base path whose first step is to a
    /// virtual base; later steps may be too).
    fn path_sub(&mut self, cls: OTy, rest: &[QualType]) -> Option<u32> {
        let i = self.o.poly_info(self.b, cls) as usize;
        let mut k = 0usize;
        for (n, &q) in rest.iter().enumerate() {
            let ast = self.b.ast();
            let d = ast.first(ast.types.as_record(q)?);
            let subs = self.o.virt.subs(i);
            // A non-virtual direct base of `k`, else the (one) virtual
            // base of the class.
            let child = (n > 0)
                .then(|| {
                    (1..subs.len()).find(|&j| {
                        let s = subs[j];
                        usize::from(s.parent) == k && !s.vbase && s.decl == d
                    })
                })
                .flatten();
            k = match child {
                Some(j) => j,
                None => subs.iter().position(|s| s.vbase && s.decl == d)?,
            };
        }
        Some(k as u32)
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
        let (off, cur, rest) = self.path_offset(e, derived);
        if !rest.is_empty() {
            // [conv.ptr]/3 through a virtual base: its offset is the most
            // derived object's ([class.mi]/4), found at run time
            // (`Oven::sub_addr`). A downcast is ill-formed
            // ([expr.static.cast]/2, /11; sema rejects it).
            let Some(k) = self.path_sub(cur, &rest).filter(|_| !down) else {
                self.unsupported(e);
                return;
            };
            let q = if off == 0 {
                p
            } else {
                let r = self.reg();
                self.emit_at(Op::Field, u32::from(ptr), &[r, p, off as u32], e);
                r
            };
            self.emit_at(Op::VBase, u32::from(ptr), &[dst, q, cur.0, k], e);
            return;
        }
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
    /// `ty` (each element of an array), if it is not trivial. An
    /// anonymous union is never destroyed on its own: the destructor of
    /// the enclosing class that would destroy it is user-provided (the
    /// implicit one would be deleted, [class.dtor]/7), and destroying it
    /// has no effect ([class.union.anon]; clang's `HandleDestructionImpl`).
    pub(super) fn dtor_of(&mut self, ty: OTy) -> Option<u32> {
        let mut t = ty;
        while let TyKind::Array { elem, .. } = self.kind(t) {
            t = elem;
        }
        let TyKind::Record { decl, union, .. } = self.kind(t) else { return None };
        let ast = self.b.ast();
        let bits = record_bits(ast, decl);
        if bits.contains(DefinitionBits::TRIVIAL_DTOR)
            || (union && bits.contains(DefinitionBits::ANONYMOUS))
        {
            return None;
        }
        let d = ast.destructor(decl)?;
        // A base-subobject view: a base class subobject of a class with
        // virtual bases, destroyed without them ([class.dtor]/13).
        if self.o.types.class_of(t) != t {
            return Some(self.o.func_index_base(self.b.ast(), d));
        }
        Some(self.o.func_index(self.b.ast(), d))
    }

    /// Whether constructor or destructor `d` has a base-object variant
    /// (its class has virtual bases).
    fn has_base_variant(&self, d: DeclId) -> bool {
        let ast = self.ast();
        ast.decls
            .get(d)
            .ctx()
            .is_some_and(|c| record_bits(ast, c).contains(DefinitionBits::HAS_VIRTUAL_BASES))
    }

    /// Registers the destruction of the object of type `ty` at `p` with
    /// the enclosing scope (aux 1: `p` is a heap array of `ty`s; aux 2: a
    /// subobject destroyed by a destructor, whose backtrace note is at the
    /// destructor's call, as clang's `HandleDestructionImpl` passes its
    /// call range down).
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
                _ => ast.exprs.children_into(e, &ast.lists, &mut work),
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
        // [class.cdtor]/4: the object's dynamic type is the class from
        // the end of its base initializers (a constructor) or the start
        // of the body (a destructor).
        let poly = record_bits(self.ast(), rdef).contains(DefinitionBits::POLYMORPHIC);
        if kind == DeclKind::CXXConstructor {
            let inits: Vec<CtorInit> = self.ast().ctor_inits(def).copied().collect();
            let delegating = inits.iter().any(|c| c.kind == CtorInitKind::Delegating);
            let mut vptr = poly && !delegating;
            let loc = self.ast().decls.get(def).loc();
            if !delegating && !union {
                self.unconstructed_bases(&inits, this, cls, loc);
            }
            // The subobjects completed so far: `done` entries of `fields`.
            let mut done = 0;
            let set = u32::from(self.base_obj);
            for ci in inits {
                if ci.kind == CtorInitKind::VirtualBase {
                    // [class.base.init]/7: only the most derived class's
                    // constructor initializes the virtual bases (sema
                    // lists them first, in `vbases()` order); an exception
                    // after one destroys it ([except.ctor]/3).
                    if !self.base_obj {
                        // The virtual base stage (`CTOR_STEP_VBASE`): no
                        // non-virtual subobject has begun its lifetime.
                        if !delegating
                            && !union
                            && let Some(j) = self.vbase_index(&ci, cls)
                        {
                            let pc = self.o.code.len() as u32;
                            self.ctor_steps.push(step(pc, crate::CTOR_STEP_VBASE | j as u32));
                        }
                        let m = self.next;
                        if let Some((q, vty)) = self.ctor_init_vbase(&ci, this, cls) {
                            self.push_cleanup(q, vty, 2 | 4, loc);
                        }
                        self.next = m;
                    }
                    continue;
                }
                if vptr && ci.kind != CtorInitKind::Base {
                    self.emit(Op::SetVptr, set, &[this, cls.0]);
                    vptr = false;
                }
                if let Some(at) = self.init_index(&ci, cls) {
                    self.unwind_subobjects(this, cls, union, done..at, loc);
                    done = done.max(at);
                    if !delegating && !union {
                        let pc = self.o.code.len() as u32;
                        self.ctor_steps.push(step(pc, at as u32));
                        // A member list-initialized records the steps of
                        // its elements (WP-465).
                        if ci.kind == CtorInitKind::Field {
                            let fi = self.o.types.fields(cls)[at];
                            self.elem_track = self
                                .tracked_list(ci.init, fi.ty)
                                .map(|list| ElemTrack { list, field: at as u32, off: fi.offset });
                        }
                    }
                }
                let m = self.next;
                self.ctor_init(&ci, this, cls, union);
                self.elem_track = None;
                self.next = m;
                if ci.kind == CtorInitKind::Delegating {
                    // [except.ctor]/4: the object is constructed; an
                    // exception from this body destroys it.
                    self.push_cleanup(this, cls, 4, loc);
                }
            }
            if vptr {
                self.emit(Op::SetVptr, set, &[this, cls.0]);
            }
            if !self.ctor_steps.is_empty() {
                let pc = self.o.code.len() as u32;
                self.ctor_steps.push(step(pc, u32::MAX));
            }
            if !delegating {
                let n = self.o.types.vbase_start(cls);
                self.unwind_subobjects(this, cls, union, done..n, loc);
            }
            return;
        }
        if poly {
            self.emit(Op::SetVptr, u32::from(self.base_obj), &[this, cls.0]);
        }
        if union {
            // A union's destructor destroys no member.
            return;
        }
        let loc = self.ast().decls.get(def).loc();
        if !self.base_obj {
            // The virtual bases, destroyed last, in the reverse of their
            // construction ([class.dtor]/13): registered first.
            let vb = self.o.types.vbase_start(cls);
            let vfields = self.o.types.fields(cls)[vb..].to_vec();
            for fi in vfields {
                let m = self.next;
                let q = self.elem_ptr_loc(this, fi.offset, loc);
                self.push_cleanup(q, fi.ty, 2, loc);
                self.next = m;
            }
        }
        let ast = self.ast();
        let bases: Vec<(QualType, bool)> =
            ast.decls.bases(rdef, &ast.lists).map(|b| (b.ty, b.is_virtual)).collect();
        for (q, virt) in bases {
            if virt {
                continue;
            }
            let bty = self.ty(q);
            let off = self.base_offset(cls, bty);
            let bty = self.o.types.nv_view(bty);
            let m = self.next;
            let q = self.elem_ptr_loc(this, off, loc);
            if self.dtor_of(bty).is_some() {
                self.push_cleanup(q, bty, 2, loc);
            } else {
                // Destroying a trivially destructible base its derived
                // class's constructor never constructed is "destroying
                // object ... whose lifetime has already ended" (clang's
                // `HandleDestructionImpl`, WP-409).
                self.emit_loc(Op::PushCleanup, 2, &[q, LIFE_CHECK, bty.0], loc);
            }
            self.next = m;
        }
        let fields = self.o.types.fields(cls).to_vec();
        for fi in fields {
            if fi.base || fi.width.is_some() || super::is_reference_decl(self.ast(), fi.decl) {
                continue;
            }
            // The destructor of an anonymous union member is never
            // invoked: its members are variant members of this class
            // ([class.dtor]/13; clang's `MarkBaseAndMemberDestructorsReferenced`).
            if let TyKind::Record { decl, union: true, .. } = self.kind(fi.ty)
                && record_bits(self.b.ast(), decl).contains(DefinitionBits::ANONYMOUS)
            {
                continue;
            }
            if self.dtor_of(fi.ty).is_none() {
                continue;
            }
            let m = self.next;
            let q = self.elem_ptr_loc(this, fi.offset, loc);
            self.push_cleanup(q, fi.ty, 2, loc);
            self.next = m;
        }
    }

    /// The direct non-virtual bases of `cls` that constructor `inits`
    /// does not initialize: Sema leaves a base out only after diagnosing
    /// that it has no usable default constructor (clang's
    /// `err_missing_default_ctor`), and clang's evaluator then leaves the
    /// base subobject without a value. Its bytes begin outside their
    /// lifetime (`EndLife` aux 8), so an access is "... object outside
    /// its lifetime" and a frozen result notes "constructor of base class
    /// 'B' is not called" (spec/oven.md "Object lifetime", WP-390). An
    /// empty base has no bytes to mark: it is recorded by identity
    /// (WP-401).
    fn unconstructed_bases(&mut self, inits: &[CtorInit], this: u32, cls: OTy, loc: Loc) {
        let covered: Vec<usize> = inits
            .iter()
            .filter(|c| c.kind == CtorInitKind::Base)
            .filter_map(|c| self.init_index(c, cls))
            .collect();
        // Not the virtual bases: the most derived constructor initializes
        // them, the others never do (P3533R2, "Virtual bases").
        let n = self.o.types.vbase_start(cls);
        let fields = self.o.types.fields(cls)[..n].to_vec();
        for (i, fi) in fields.into_iter().enumerate() {
            if !fi.base || covered.contains(&i) || self.o.types.holds_no_data(fi.ty) {
                continue;
            }
            let m = self.next;
            let q = self.elem_ptr_loc(this, fi.offset, loc);
            self.emit_loc(Op::EndLife, 8, &[q, fi.ty.0], loc);
            self.next = m;
        }
        // Empty bases are kept apart from `fields` (aux 16: no bytes).
        let ebases = self.o.types.empty_bases(cls).to_vec();
        if ebases.is_empty() {
            return;
        }
        let inited: Vec<OTy> = inits
            .iter()
            .filter(|c| c.kind == CtorInitKind::Base)
            .map(|c| self.ty(QualType::unqual(TypeId::new(c.target))))
            .collect();
        for (bt, off) in ebases {
            if inited.contains(&bt) {
                continue;
            }
            let m = self.next;
            let q = self.elem_ptr_loc(this, off, loc);
            self.emit_loc(Op::EndLife, 8 | 16, &[q, bt.0], loc);
            self.next = m;
        }
    }

    /// The index in `fields(cls)` of the subobject ctor-initializer `ci`
    /// initializes (for a member of an anonymous member, that member).
    fn init_index(&mut self, ci: &CtorInit, cls: OTy) -> Option<usize> {
        let fd = match ci.kind {
            CtorInitKind::Base => {
                let bty = self.ty(QualType::unqual(TypeId::new(ci.target)));
                let t = &self.o.types;
                return t.fields(cls)[..t.vbase_start(cls)]
                    .iter()
                    .position(|f| f.base && t.same_class(f.ty, bty));
            }
            CtorInitKind::Field => DeclId::new(ci.target),
            CtorInitKind::IndirectField => {
                let ast = self.ast();
                let chain = ast.indirect_field_chain(DeclId::new(ci.target));
                DeclId::new(*ast.lists.get(chain).first()?)
            }
            CtorInitKind::VirtualBase | CtorInitKind::Delegating => return None,
        };
        self.o.types.fields(cls).iter().position(|f| !f.base && f.decl == fd)
    }

    /// Registers, as destructions run only by unwinding, the completed
    /// subobjects `range` of `fields(cls)` of the object under
    /// construction at `this` ([except.ctor]/3: an exception from a later
    /// initializer or the constructor's body destroys them, in reverse
    /// order, before a handler of its function-try-block is entered).
    /// The members of a union, and anonymous unions, are not destroyed
    /// (as by the destructor).
    fn unwind_subobjects(
        &mut self,
        this: u32,
        cls: OTy,
        union: bool,
        range: std::ops::Range<usize>,
        loc: Loc,
    ) {
        if union || range.is_empty() {
            return;
        }
        let fields = self.o.types.fields(cls)[range].to_vec();
        for fi in fields {
            if fi.width.is_some() || !fi.base && super::is_reference_decl(self.ast(), fi.decl) {
                continue;
            }
            if let TyKind::Record { decl, union: true, .. } = self.kind(fi.ty)
                && record_bits(self.b.ast(), decl).contains(DefinitionBits::ANONYMOUS)
            {
                continue;
            }
            if self.dtor_of(fi.ty).is_none() {
                continue;
            }
            let m = self.next;
            let q = self.elem_ptr_loc(this, fi.offset, loc);
            self.push_cleanup(q, fi.ty, 2 | 4, loc);
            self.next = m;
        }
    }

    /// Initializes the virtual base of the complete object at `this` (of
    /// class `cls`) that ctor-initializer `ci` names, at its offset in
    /// `cls`; its address and type (a base-subobject view if it has
    /// virtual bases of its own), `None` for an empty one (not laid out:
    /// its constructor still runs, on the object's address).
    /// The index among the virtual bases of `cls` (entries from
    /// `vbase_start`) of the one virtual-base initializer `ci` initializes.
    fn vbase_index(&mut self, ci: &CtorInit, cls: OTy) -> Option<usize> {
        let vty = self.ty(QualType::unqual(TypeId::new(ci.target)));
        let t = &self.o.types;
        t.fields(cls)[t.vbase_start(cls)..].iter().position(|f| t.same_class(f.ty, vty))
    }

    fn ctor_init_vbase(&mut self, ci: &CtorInit, this: u32, cls: OTy) -> Option<(u32, OTy)> {
        let vty = self.ty(QualType::unqual(TypeId::new(ci.target)));
        let vb = self.o.types.vbase_start(cls);
        let found = self.o.types.fields(cls)[vb..]
            .iter()
            .find(|f| self.o.types.same_class(f.ty, vty))
            .map(|f| (f.offset, f.ty));
        let (off, ty) = found.unwrap_or((0, self.o.types.nv_view(vty)));
        let q = self.elem_ptr(this, off, ci.init);
        self.init(ci.init, q, ty);
        found.map(|_| (q, ty))
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
                // The base-subobject view: zero-initialization
                // ([dcl.init.general]/9) stops at its virtual bases.
                let bty = self.o.types.nv_view(bty);
                self.init(ci.init, q, bty);
            }
            CtorInitKind::VirtualBase => {
                let _ = self.ctor_init_vbase(ci, this, cls);
            }
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

    /// The parameters of `callee`'s first declaration (none for a call
    /// through a pointer).
    fn callee_params(&self, callee: Callee) -> Vec<DeclId> {
        let ast = self.ast();
        let (Callee::Direct(d) | Callee::Base(d) | Callee::Virtual(d, _)) = callee else {
            return Vec::new();
        };
        ast.decls
            .function(ast.first(d))
            .map(|f| ast.lists.get(f.params).iter().map(|&p| DeclId::new(p)).collect())
            .unwrap_or_default()
    }

    /// clang `isValidVarArgType` returning `VAK_Undefined` (C++11 on):
    /// class type `q` has a non-trivial copy constructor, move
    /// constructor or destructor.
    fn vararg_non_trivial(&self, q: QualType) -> bool {
        let ast = self.ast();
        let Some(r) = ast.types.as_record(q) else { return false };
        let bits = record_bits(ast, r);
        !bits.contains(DefinitionBits::TRIVIAL_COPY_CTOR)
            || !bits.contains(DefinitionBits::TRIVIAL_MOVE_CTOR)
            || !bits.contains(DefinitionBits::TRIVIAL_DTOR)
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
        if self.cur_fn.is_some() && self.has_virtual_bases(e) && !self.constructs_literal(e) {
            // clang `CheckLiteralType`: an object of a class with a
            // virtual base (not a literal type) cannot be created in a
            // function called during evaluation; only the object being
            // initialized by the evaluated declaration itself may be.
            // Where clang 22 skips that check (C++23), a constexpr
            // constructor of such a class (all its virtual bases
            // indirect) fails at `HandleConstructorCall`'s virtual-base
            // check instead; a non-constexpr one fails at the call.
            let q = ast.exprs.get(e).ty().without_quals();
            let t = crate::Oven::type_name(ast, q);
            if !self.literal_check_relaxed() {
                self.trap(
                    e,
                    &format!("non-literal type '{t}' cannot be used in a constant expression"),
                );
                return;
            }
            let ctor = ast.exprs.constructor(e);
            if ast
                .decls
                .function(ast.latest(ctor))
                .is_some_and(|f| f.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None)
            {
                self.trap(
                    e,
                    &format!(
                        "cannot construct object of type '{t}' with virtual base class in a constant expression"
                    ),
                );
                return;
            }
        }
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

    /// The class (or array element class) `e` constructs has a virtual
    /// base.
    fn has_virtual_bases(&self, e: ExprId) -> bool {
        let ast = self.ast();
        let ctor = ast.exprs.constructor(e);
        ast.decls
            .get(ctor)
            .ctx()
            .is_some_and(|c| record_bits(ast, c).contains(DefinitionBits::HAS_VIRTUAL_BASES))
    }

    /// The class `e` constructs is a literal type (C++26 lets a class
    /// with virtual bases be one, P3533R2).
    fn constructs_literal(&self, e: ExprId) -> bool {
        let ast = self.ast();
        let ctor = ast.exprs.constructor(e);
        ast.decls
            .get(ctor)
            .ctx()
            .is_some_and(|c| record_bits(ast, c).contains(DefinitionBits::LITERAL))
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
            self.emit_ptr_add(int_aux(64, false), &[q, p, k, elem.0], e);
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
                // representation ([class.copy.ctor]/14); a class's
                // source is read whole, so it must be fully initialized
                // (aux 2; clang's `handleTrivialCopy`).
                [a] => {
                    let a = skip_parens(self.ast(), a);
                    if self.node(a).value_category().is_glvalue() {
                        // A class holding no data is not read (clang's
                        // `isReadByLvalueToRvalueConversion`): copying an
                        // `extern const` empty object is a constant.
                        let read = !self.o.types.holds_no_data(ty);
                        let src = self.lv_mem(a, if read { Access::Read } else { Access::Addr });
                        let full = matches!(self.kind(ty), TyKind::Record { union: false, .. });
                        let aux = 1 | 4 | if full { 2 } else { 0 };
                        self.emit_at(Op::Copy, aux, &[p, src, ty.0], e);
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
        // A base class subobject (or a delegation from one) is
        // constructed without its virtual bases ([class.base.init]/7).
        let callee = match self.ast().exprs.construct_info(e).kind {
            ConstructionKind::NonVirtualBase | ConstructionKind::VirtualBase
                if self.has_base_variant(ctor) =>
            {
                Callee::Base(ctor)
            }
            ConstructionKind::Delegating if self.base_obj => Callee::Base(ctor),
            _ => Callee::Direct(ctor),
        };
        self.call_frame(e, callee, void, None, Some(p), &args, &refs, Some(loc));
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
        let f = if self.has_base_variant(ctor) {
            self.o.func_index_base(self.b.ast(), ctor)
        } else {
            self.o.func_index(self.b.ast(), ctor)
        };
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
        // A call returning a reference (a glvalue) returns the referent's
        // address in a register, whatever the referent's type.
        let is_sret = self.o.types.is_aggregate(rty) && !self.node(e).value_category().is_glvalue();
        let adj = match callee {
            Callee::Virtual(d, _) if self.returns_class_ref(d) => Some(self.reg()),
            _ => None,
        };
        // The parameter objects' addresses, kept below the call window
        // (the result overwrites its first register): their lifetime ends
        // when the call returns (`EndParam` below).
        let by_value = args
            .iter()
            .enumerate()
            .filter(|&(i, &a)| {
                !refs.get(i).copied().unwrap_or(false) && {
                    let aty = self.ety(a);
                    self.o.types.is_aggregate(aty)
                }
            })
            .count() as u32;
        let keep = self.next;
        for _ in 0..by_value {
            self.reg();
        }
        let mut kept: Vec<(u32, bool)> = Vec::new();
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
        let params = self.callee_params(callee);
        for (i, &a) in args.iter().enumerate() {
            let slot = base + first + i as u32;
            let aty = self.ety(a);
            if refs.get(i).copied().unwrap_or(false) {
                self.bind_ref(a, slot);
            } else if self.o.types.is_aggregate(aty) {
                let q = self.ast().exprs.get(a).ty().without_quals();
                if !self.literal_check_relaxed() && !super::is_literal_type(self.ast(), q) {
                    // clang `EvaluateCallArg` -> `EvaluateInPlace` ->
                    // `CheckLiteralType`: a by-value argument (to a
                    // declared or a variadic parameter) is a prvalue
                    // initializing a parameter object, never the object
                    // of the evaluated declaration.
                    let t = crate::Oven::type_name(self.ast(), q);
                    self.trap(
                        a,
                        &format!("non-literal type '{t}' cannot be used in a constant expression"),
                    );
                } else if i >= refs.len() && self.vararg_non_trivial(q) {
                    // [expr.call]/12: passing a class with a non-trivial
                    // copy or move constructor or destructor through the
                    // ellipsis is conditionally-supported; clang
                    // (`DefaultVariadicArgumentPromotion`) makes the
                    // argument `(__builtin_trap(), a)`, which no constant
                    // evaluation gets past.
                    let at = self.begin(a);
                    self.trap_loc(at, super::builtins::INVALID.into());
                }
                // [expr.call]/7: the parameter object, named (and
                // located, for its destructor's "in call to" note) by
                // the callee's first declaration of the parameter, as
                // clang's canonicalized `APValue` base does.
                let (origin, at) = match params.get(i) {
                    Some(&pd) => (Origin::Var(pd), self.ast().decls.get(pd).loc()),
                    None => (Origin::Temp(a), self.begin(a)),
                };
                self.alloc(slot, aty, origin, false, a);
                let k = keep + kept.len() as u32;
                self.emit(Op::Mov, 0, &[k, slot]);
                kept.push((k, self.dtor_of(aty).is_some()));
                self.init(a, slot, aty);
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
            Callee::Base(d) => {
                let f = self.o.func_index_base(self.b.ast(), d);
                self.emit_at(Op::Call, 0, &[f, base], e)
            }
            Callee::Ind(r) => self.emit_at(Op::CallInd, 0, &[r, base], e),
            Callee::Virtual(d, cls) => {
                let m = self.ast().first(d);
                let a = adj.unwrap_or(u32::MAX);
                self.emit_at(Op::VCall, u32::from(is_sret), &[m.index(), base, cls.0, a], e)
            }
        };
        if let Some(a) = adj {
            self.emit(Op::CovAdj, 0, &[base, a]);
        }
        if let Some(loc) = loc
            && let Some(site) = self.o.sites.last_mut().filter(|x| x.pc == pc as u32)
        {
            site.loc = loc;
        }
        // [expr.call]/7: the parameter objects are destroyed when the
        // function returns (implementation-defined; clang's `CallScope`),
        // last constructed first, and a pointer to one dangles from then
        // on.
        for &(k, dtor) in kept.iter().rev() {
            self.emit_at(Op::EndParam, u32::from(dtor), &[k], e);
        }
        self.next = base + 1;
        base
    }

    /// Whether function `d` returns a pointer or reference to a class (a
    /// virtual one may be overridden covariantly, [class.virtual]/8).
    fn returns_class_ref(&self, d: DeclId) -> bool {
        let ast = self.ast();
        let def = ast.definition(d).unwrap_or(d);
        let Some(fty) = ast.decls.get(def).ty() else { return false };
        let kiln_ast::types::TypeView::Function(p) = ast.types.canonical_view(fty) else {
            return false;
        };
        ast.types.pointee(p.ret()).and_then(|r| ast.types.as_record(r)).is_some()
    }

    /// Whether the object expression `obj` of a member call designates an
    /// object whose dynamic type is its static type (so a virtual call
    /// needs no dispatch): a variable or temporary of class type. (`*this`
    /// in a constructor or destructor dispatches: the final overrider in
    /// the constructor's class may be pure or in another base.)
    fn exact_dynamic_type(&self, obj: ExprId, arrow: bool) -> bool {
        let ast = self.ast();
        let mut x = skip_parens(ast, obj);
        while ast.exprs.get(x).kind().is_cast() && ast.exprs.cast_kind(x) == CastKind::NoOp {
            x = skip_parens(ast, ast.exprs.operand(x));
        }
        let n = ast.exprs.get(x);
        match n.kind() {
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
        let dispatch = spec.is_some_and(|s| s.is_virtual() && !s.is_final())
            && !qualified
            && !class_final
            && !self.exact_dynamic_type(obj, arrow);
        // [class.union.general]/6: `u.m = x` by a trivial assignment
        // operator makes `m` (and each union member on the access chain)
        // active, as a built-in assignment does (clang's
        // `MaybeHandleUnionActiveMemberChange`); only the operator syntax
        // does: `u.m.operator=(x)` is a member call on `m` (WP-224).
        let trivial_assign = self.node(e).kind() == ExprKind::CXXOperatorCallExpr
            && spec.is_some_and(kiln_ast::decl::FnSpec::is_trivial)
            && self.ast().decls.get(md).name().as_operator()
                == Some(kiln_ast::names::OverloadedOperator::Equal);
        let this = if arrow {
            self.rv(obj)
        } else if self.node(obj).value_category().is_glvalue() {
            self.lv_mem(obj, if trivial_assign { Access::Write } else { Access::Addr })
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
        let callee_e = callee;
        let cls = self.method_class(md);
        let callee = match cls {
            Some(cls) if dispatch => Callee::Virtual(md, cls),
            _ => Callee::Direct(md),
        };
        // An explicit destructor call ends the object's lifetime
        // ([basic.life]/1.4; spec/oven.md "Transient allocation").
        let dtor = cls.filter(|_| self.ast().decls.get(md).kind() == DeclKind::CXXDestructor);
        if let Some(cls) = cls {
            // clang diagnoses both at the member name (`s.~S()`: `~S`).
            let (aux, at) = (if dtor.is_some() { 1 } else { 4 }, self.node(callee_e).loc());
            self.emit_site(Op::EndLife, aux, &[this, cls.0], e, at);
        }
        // An explicit call of a trivial destructor (`p->~W()`) runs nothing
        // ([class.dtor]/14, /18; WP-215): only the lifetime bookkeeping
        // happens, as for a non-trivial one.
        if let Some(cls) = dtor
            && spec.is_some_and(kiln_ast::decl::FnSpec::is_trivial)
        {
            self.emit_site(Op::EndLife, 2, &[this, cls.0], e, loc);
            return self.reg();
        }
        // A trivial copy/move assignment operator, called by either
        // syntax, copies the object (clang's `handleTrivialCopy`): a
        // union's with its active member (sema defines it with an empty
        // body), a class's from a fully initialized source (aux 2; noted
        // at the argument, the call's frame note at the call).
        if spec.is_some_and(kiln_ast::decl::FnSpec::is_trivial)
            && self.ast().decls.get(md).name().as_operator()
                == Some(kiln_ast::names::OverloadedOperator::Equal)
            && let (Some(cls), [src]) = (cls, args)
            && let TyKind::Record { union, .. } = self.kind(cls)
        {
            let read = !self.o.types.holds_no_data(cls);
            let s = self.lv_mem(*src, if read { Access::Read } else { Access::Addr });
            self.emit_site(Op::Copy, 4 | if union { 0 } else { 2 }, &[this, s, cls.0], e, loc);
            return this;
        }
        let r = self.call_frame(e, callee, rty, sret, Some(this), args, &refs, Some(loc));
        if let Some(cls) = dtor {
            self.emit_site(Op::EndLife, 2, &[this, cls.0], e, loc);
        }
        r
    }

    /// The class type of member `md`.
    pub(super) fn method_class(&mut self, md: DeclId) -> Option<OTy> {
        let ast = self.ast();
        let q = ast.decls.get(md).ctx().and_then(|c| ast.decls.get(c).ty())?;
        Some(self.ty(q))
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
        self.emit_ptr_add(int_aux(64, false), &[q, p, k, elem.0], e);
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

/// A constructor step that is not an element's ([`crate::CtorStep`]).
fn step(pc: u32, cur: u32) -> crate::CtorStep {
    crate::CtorStep { pc, cur, off: 0, len: 0 }
}
