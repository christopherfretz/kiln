//! Splices of value and object reflections (`-freflection`;
//! spec/reflection.md §5; [expr.prim.splice]/2, P2996R13; WP-550).
//!
//! A value reflection (`reflect_constant` of a scalar, a non-class
//! constant template argument) splices to a prvalue of the value's type:
//! the expression substituting that value for a template parameter
//! stands for ([`Sema::nttp_arg_expr`]). An object reflection
//! (`reflect_object`, a class-type `reflect_constant` or template
//! argument: its template parameter object) splices to an lvalue naming
//! the object: a reference to the variable or template parameter object
//! holding it, followed by the member accesses, derived-to-base
//! conversions and subscripts that reach the subobject its byte offset
//! designates ([`Sema::object_lvalue`]). Substituting a pointer or
//! reference template argument into a subobject uses the same walk.

use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, ExprFlags, NonOdrUse, UnaryOp};
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_source::Loc;

use crate::Sema;

/// How deep [`Sema::subobject_path`] descends at most (a bound on a
/// malformed type; a real class nests far less).
const MAX_DEPTH: u32 = 1024;

/// One step from an object to one of its subobjects.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// The non-static data member.
    Field(DeclId),
    /// A direct non-virtual or a virtual base class (by its first
    /// declaration) of the current class (by its definition).
    Base { derived: DeclId, base: DeclId },
    /// The array element.
    Index(u64),
}

impl Sema<'_, '_> {
    /// `[: r :]` of value reflection `v` ([expr.prim.splice]/2): a prvalue
    /// of the value's type computing it. `None` for a value kiln cannot
    /// spell (the caller diagnoses).
    pub(crate) fn splice_value_expr(&mut self, v: ConstValueId, loc: Loc) -> Option<ExprId> {
        let ty = self.ast.consts.ty(v);
        self.nttp_arg_expr(ty, v, loc)
    }

    /// `[: r :]` of object reflection `p` (its interned pointer to the
    /// object; [expr.prim.splice]/2): an lvalue designating the object,
    /// of the object's type. `None` for an object with no declaration
    /// to name it through (the caller diagnoses).
    ///
    /// The object is not a variable, so a contract predicate does not
    /// make the splice `const` (P3598R0, [expr.prim.splice]/2.3: "the
    /// expression has the same type as that of S"; only a splice
    /// designating a variable or structured binding is const-ified,
    /// /2.5). `[: object_of(^^v) :]` is the paper's deliberate, visible
    /// way around the const-ification, so the reference to the variable
    /// holding the object is built outside the contract context.
    pub(crate) fn splice_object_expr(&mut self, p: ConstValueId, loc: Loc) -> Option<ExprId> {
        let Some(Scalar::Pointer { ty, base, offset, .. }) = self.ast.consts.scalar(p) else {
            return None;
        };
        let target = self.ast.types.pointee(ty)?;
        let contracts = std::mem::take(&mut self.contract_ctx);
        let r = self.object_lvalue(base, offset, target, loc);
        self.contract_ctx = contracts;
        r
    }

    /// What a splice of object reflection `p` that
    /// [`Self::splice_object_expr`] could not name is, for the
    /// not-supported diagnostic.
    pub(crate) fn object_splice_unsupported(&self, p: ConstValueId) -> &'static str {
        match self.ast.consts.scalar(p) {
            Some(Scalar::Pointer { base: PtrBase::Const(_), .. }) => {
                "splice of a reflection of an object with no declaration \
                 ('reflect_constant_string', 'reflect_constant_array')"
            }
            Some(Scalar::Pointer { base: PtrBase::StringLit { .. }, .. }) => {
                "splice of a reflection of a string literal object"
            }
            Some(Scalar::Pointer { base: PtrBase::TypeInfo(_), .. }) => {
                "splice of a reflection of a 'std::type_info' object"
            }
            _ => "splice of a reflection of this subobject",
        }
    }

    /// An lvalue designating the object of type `target` at byte `offset`
    /// of `base`: the variable or template parameter object, then each
    /// step to the subobject. `None` when `base` is not a declared object
    /// or no subobject of that type lies at `offset`.
    pub(crate) fn object_lvalue(
        &mut self,
        base: PtrBase,
        offset: u64,
        target: QualType,
        loc: Loc,
    ) -> Option<ExprId> {
        let PtrBase::Decl(d) = base else { return None };
        let mut cur = self.object_decl_ref(d, loc)?;
        let root = self.ety(cur);
        let mut path = Vec::new();
        if !self.subobject_path(root, offset, target, true, &mut path, 0) {
            return None;
        }
        for step in path {
            cur = match step {
                Step::Field(f) => {
                    let q = self.ast.types.effective_quals(self.ety(cur));
                    self.field_access(cur, f, q, false, loc)
                }
                Step::Base { derived, base } => {
                    let q = self.ast.types.effective_quals(self.ety(cur));
                    self.object_to_class_as(
                        cur,
                        false,
                        derived,
                        q,
                        (base, None),
                        q,
                        CastKind::UncheckedDerivedToBase,
                    )
                }
                Step::Index(i) => {
                    let st = self.size_type();
                    let idx = self.nttp_int_literal(st, i128::from(i), loc);
                    self.act_on_subscript(cur, idx, loc)
                }
            };
            if self.is_err_expr(cur) {
                return None;
            }
        }
        Some(cur)
    }

    /// The expression a pointer or reference template argument of type
    /// `ty` designating a subobject (`pty`, `base`, `offset`;
    /// [`Self::is_subobject_ptr`]) is substituted as: the lvalue naming
    /// the subobject (`v.m[i]`), its address unless `ty` is a reference.
    pub(crate) fn subobject_arg_expr(
        &mut self,
        ty: QualType,
        pty: QualType,
        base: PtrBase,
        offset: u64,
        loc: Loc,
    ) -> Option<ExprId> {
        let target = self.ast.types.pointee(pty)?;
        let lv = self.object_lvalue(base, offset, target, loc)?;
        if self.ast.types.is_reference(ty) {
            return Some(lv);
        }
        let a = self.act_on_unary(UnaryOp::AddrOf, lv, loc);
        if self.is_err_expr(a) {
            return None;
        }
        if self.ast.types.canonical(self.ety(a)) == self.ast.types.canonical(ty) {
            Some(a)
        } else {
            crate::conv::implicit_convert(self, a, ty, crate::conv::ConvCtx::Initialization).ok()
        }
    }

    /// Whether pointer value (`pty`, `base`, `offset`) designates a proper
    /// subobject of a declared object rather than the object itself (or
    /// its first array element, which an array argument decays to).
    pub(crate) fn is_subobject_ptr(&self, pty: QualType, base: PtrBase, offset: u64) -> bool {
        let PtrBase::Decl(d) = base else { return false };
        if offset != 0 {
            return true;
        }
        let (Some(target), Some(dt)) = (self.ast.types.pointee(pty), self.ast.decls.get(d).ty())
        else {
            return false;
        };
        let t = &self.ast.types;
        if t.is_void(target) || t.same_unqualified(dt, target) {
            return false;
        }
        !t.element(dt).is_some_and(|e| t.is_array(dt) && t.same_unqualified(e, target))
    }

    /// A reference to the object `d` declares: a variable (as an
    /// id-expression names it) or a template parameter object (of its
    /// `const` type).
    fn object_decl_ref(&mut self, d: DeclId, loc: Loc) -> Option<ExprId> {
        match self.kind_of(d) {
            DeclKind::TemplateParamObject => {
                let q = self.ast.decls.get(d).ty()?;
                Some(self.builder().decl_ref(d, q, loc, None, NonOdrUse::None, ExprFlags::NONE))
            }
            DeclKind::Var | DeclKind::VarTemplateSpecialization => {
                let r = self.build_decl_ref(d, loc);
                (!self.is_err_expr(r)).then_some(r)
            }
            _ => None,
        }
    }

    /// Appends to `path` the steps from an object of type `ty` to its
    /// subobject of type `target` at byte `offset` (nothing when the
    /// object itself is meant). Bases are tried before members, and
    /// members sharing an offset (an empty base, a union's members, a
    /// `[[no_unique_address]]` member) in order, until one holds
    /// `target`. A `complete` object (not a base class subobject) also
    /// holds its virtual bases, at the complete-object offsets the layout
    /// gives them. False when none does.
    fn subobject_path(
        &mut self,
        ty: QualType,
        offset: u64,
        target: QualType,
        complete: bool,
        path: &mut Vec<Step>,
        depth: u32,
    ) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }
        if offset == 0 && self.ast.types.same_unqualified(ty, target) {
            return true;
        }
        if self.ast.types.is_array(ty) {
            let (Some(elem), Some(len)) =
                (self.ast.types.element(ty), self.ast.types.array_size(ty))
            else {
                return false;
            };
            let Some((stride, _)) = crate::target::size_align(&self.ast, elem, &self.abi) else {
                return false;
            };
            if stride == 0 {
                return false;
            }
            let i = offset / stride;
            if i >= len {
                return false;
            }
            path.push(Step::Index(i));
            if self.subobject_path(elem, offset - i * stride, target, true, path, depth + 1) {
                return true;
            }
            path.pop();
            return false;
        }
        let Some(rec) = self.ast.types.as_record(ty) else { return false };
        let Some(def) = self.ast.definition(rec) else { return false };
        let Some(lay) = kiln_abi::ensure_layout(&mut self.ast, def, &self.abi) else {
            return false;
        };
        let mut bases = lay.bases.to_vec();
        if complete {
            bases.extend(lay.vbases.iter().copied());
        }
        let fields = lay.fields.clone();
        for (b, at) in bases {
            let Some(bdef) = self.ast.definition(b) else { continue };
            let Some(size) =
                kiln_abi::ensure_layout(&mut self.ast, bdef, &self.abi).map(|l| l.nv_size)
            else {
                continue;
            };
            if at > offset || (offset >= at + size && !(offset == at && size == 0)) {
                continue;
            }
            let bt = self.tag_type(bdef);
            path.push(Step::Base { derived: def, base: self.ast.first(b) });
            if self.subobject_path(bt, offset - at, target, false, path, depth + 1) {
                return true;
            }
            path.pop();
        }
        let members: Vec<DeclId> =
            self.ast.members(def).filter(|&m| self.kind_of(m) == DeclKind::Field).collect();
        for (f, bits) in members.into_iter().zip(fields.iter().copied()) {
            let Some(data) = self.ast.decls.field(f) else { continue };
            if data.bit_width.is_some() || bits % 8 != 0 {
                continue;
            }
            let Some(ft) = self.ast.decls.get(f).ty() else { continue };
            if self.ast.types.is_reference(ft) {
                continue;
            }
            let at = bits / 8;
            let Some((size, _)) = crate::target::size_align(&self.ast, ft, &self.abi) else {
                continue;
            };
            if at > offset || (offset >= at + size && !(offset == at && size == 0)) {
                continue;
            }
            path.push(Step::Field(f));
            if self.subobject_path(ft, offset - at, target, true, path, depth + 1) {
                return true;
            }
            path.pop();
        }
        false
    }
}
