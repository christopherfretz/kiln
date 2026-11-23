//! Parameter attributes across declarations (spec/sema.md §4.5
//! "Parameter attributes", WP-2476): the attributes a redeclaration's
//! parameter inherits from the previous declaration's (clang
//! `mergeParamDeclAttributes`, the `InheritableParamAttr` kinds), the
//! `noescape` mismatch that makes two C++ declarations' types conflict
//! (clang's `ExtParameterInfo`), the attributes of a parameter of a
//! function type inside a declarator (clang `ProcessDeclAttributes` on
//! every `ParmVarDecl`), and a value-dependent `align_value` at
//! instantiation (clang `instantiateDependentAlignValueAttr`).

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::DeclFlags;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;

use crate::Sema;
use crate::declspec::ParamInfo;

impl Sema<'_, '_> {
    /// Whether a parameter inherits an attribute of kind `k` from the
    /// corresponding parameter of the previous declaration: clang's
    /// `InheritableParamAttr` kinds kiln implements (`nonnull`,
    /// `annotate`, `carries_dependency`, the Swift parameter ABIs;
    /// `lifetimebound` from clang 22, an `InheritableParamAttr` there and
    /// a plain attribute in clang 19). `pass_object_size` is one too, but
    /// a declaration whose parameter differs in it declares another
    /// overload in clang, so it never has one to inherit.
    fn inherits_param_attr(&self, k: AttrKind) -> bool {
        matches!(
            k,
            AttrKind::NONNULL
                | AttrKind::ANNOTATE
                | AttrKind::CARRIES_DEPENDENCY
                | AttrKind::SWIFT_PARAM_ABI
        ) || (k == AttrKind::LIFETIMEBOUND && self.clang_compat >= 22)
    }

    /// clang `DeclHasAttr`: parameter `p` already has an attribute of
    /// `n`'s kind (an `annotate` one with the same string).
    fn param_has_attr_like(&self, p: DeclId, n: &AttrNode) -> bool {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(p))
            .any(|a| self.same_param_attr(self.ast.attrs.get(a), n))
    }

    /// clang `DeclHasAttr`'s comparison: the same kind, and for
    /// `annotate` the same string (the other arguments are not
    /// compared).
    fn same_param_attr(&self, m: &AttrNode, n: &AttrNode) -> bool {
        if m.kind != n.kind {
            return false;
        }
        if n.kind != AttrKind::ANNOTATE {
            return true;
        }
        let text = |x: &AttrNode| {
            ExprId::from_raw(x.a)
                .and_then(|e| self.string_lit_of(e))
                .map(|l| self.ast.strings.bytes(self.ast.exprs.string_lit(l)).to_vec())
        };
        text(m) == text(n)
    }

    /// clang `mergeParamDeclAttributes` for every parameter of function
    /// declaration `f`, a redeclaration of `prev` (the declaration its
    /// lookup found): each inherits, marked [`AttrNode::INHERITED`], the
    /// inheritable attributes of `prev`'s corresponding parameter (its
    /// own and those it inherited) of a kind it does not have yet. Not
    /// when the parameter counts differ (clang: a K&R definition).
    pub(crate) fn merge_param_attrs(&mut self, f: DeclId, prev: DeclId) {
        let params = |s: &Self, d: DeclId| -> Vec<DeclId> {
            s.ast
                .decls
                .function(d)
                .map(|x| s.ast.lists.iter(x.params).collect())
                .unwrap_or_default()
        };
        let (new, old) = (params(self, f), params(self, prev));
        if new.len() != old.len() {
            return;
        }
        for (&np, &op) in new.iter().zip(&old) {
            if !self.ast.decls.get(op).has(DeclFlags::HAS_ATTRS) {
                continue;
            }
            let mut extra = Vec::new();
            let olds: Vec<AttrNode> = self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(op))
                .map(|a| *self.ast.attrs.get(a))
                .collect();
            for n in olds {
                if !self.inherits_param_attr(n.kind) || self.param_has_attr_like(np, &n) {
                    continue;
                }
                if extra.iter().any(|&a: &AttrId| self.same_param_attr(self.ast.attrs.get(a), &n)) {
                    continue;
                }
                let copy = AttrNode { flags: n.flags | AttrNode::INHERITED, ..n };
                extra.push(self.ast.attrs.push(copy));
            }
            self.builder().append_attrs(np, &extra);
        }
    }

    /// Whether parameter type `ty` keeps a `noescape` (clang
    /// `handleNoEscapeAttr`: a pointer, a reference, or dependent).
    fn noescape_applies(&self, ty: QualType) -> bool {
        let t = &self.ast.types;
        t.is_pointer(ty) || t.is_reference(ty) || t.is_dependent(ty)
    }

    /// Whether the `noescape` of some parameter of function declarator
    /// `params` differs from that of the corresponding parameter of the
    /// previous declaration `prev`: in C++ the two function types differ
    /// (clang's `ExtParameterInfo`, compared by `MergeFunctionDecl`), so
    /// the redeclaration is "conflicting types" (clang 19 and 22). C
    /// merges them into a composite type instead (clang
    /// `mergeExtParameterInfo`).
    pub(crate) fn noescape_conflicts(&self, prev: DeclId, params: &[ParamInfo]) -> bool {
        if self.is_c() {
            return false;
        }
        let Some(pf) = self.ast.decls.function(prev) else { return false };
        let old: Vec<DeclId> = self.ast.lists.iter(pf.params).collect();
        if old.len() != params.len() {
            return false;
        }
        params.iter().zip(&old).any(|(p, &op)| {
            let new = self.noescape_applies(p.ty)
                && p.attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::NOESCAPE);
            let was = self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(op))
                .any(|a| self.ast.attrs.get(a).kind == AttrKind::NOESCAPE);
            new != was
        })
    }

    /// The attributes of the parameters of a function type inside a
    /// declarator (`void (*f(int))(int *y [[gnu::nonnull]])`, a
    /// typedef's, a type-id's), checked and kept on each parameter's own
    /// declaration as clang keeps them on the `ParmVarDecl` of the
    /// `FunctionProtoTypeLoc` (clang `ActOnParamDeclarator` →
    /// `ProcessDeclAttributes`). A parameter a function adopts gets its
    /// attributes from `Sema::create_params` instead.
    pub(crate) fn nested_param_attrs(&mut self, params: &[ParamInfo]) {
        for p in params {
            let Some(pd) = p.decl else { continue };
            if p.attrs.is_empty()
                || !self.unowned_params.contains(&pd)
                || self.ast.decls.get(pd).has(DeclFlags::HAS_ATTRS)
            {
                continue;
            }
            self.attach_param_attrs(pd, p);
        }
    }

    /// A value-dependent `align_value(N)` (`node`, its `N` substituted
    /// as `e`) on an instantiated declaration (clang
    /// `instantiateDependentAlignValueAttr` → `AddAlignValueAttr`): still
    /// dependent, kept so; else `N` must be an integer constant ("'X'
    /// attribute requires an integer constant") and a power of two
    /// ("requested alignment is not a power of 2"); `None` after an
    /// error.
    pub(crate) fn instantiate_align_value(
        &mut self,
        node: AttrNode,
        e: ExprId,
    ) -> Option<AttrNode> {
        if self.expr_is_value_dependent(e) {
            return Some(AttrNode { a: e.raw(), ..node });
        }
        let Some(v) = self.fold_int(e) else {
            let q = format!("'{}'", self.attr_shown(&node, "align_value"));
            let msg = self.diag(DiagId::sema_attribute_requires_integer_constant, node.loc).arg(q);
            self.emit(msg);
            return None;
        };
        let u = u64::try_from(v).unwrap_or(0);
        if !u.is_power_of_two() {
            self.error(DiagId::sema_alignment_not_power_of_two, node.loc);
            return None;
        }
        let flags = node.flags & !AttrNode::ALIGN_VALUE_DEPENDENT;
        Some(AttrNode { flags, a: u32::try_from(u).unwrap_or(0), ..node })
    }
}
