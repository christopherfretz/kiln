//! GCC's `__builtin_has_attribute (type-or-expression, attribute)`
//! (spec/sema.md §5.4; GCC `has_attribute` in `c-attribs.cc`; WP-1611).
//!
//! The result is a `bool` constant: whether the declaration or type the
//! first operand names carries the attribute, answered from the
//! attributes kiln stores. The probe attribute is built like a written
//! one (so its arguments are checked with the usual diagnostics) but
//! never attached. An expression operand is looked through parentheses
//! and implicit conversions: a name or member access asks its
//! declaration (every redeclaration) first, then the declaration's type;
//! any other expression asks its type. A type is asked through its
//! sugar: the typedefs it was named by, then its class or enumeration;
//! pointers, references and functions stop the walk (the attribute of a
//! pointer is not the pointee's). An attribute kiln parses and drops
//! answers 0; of those it accepts silently, `nonstring` and `nocommon`
//! are kept for this query where GCC keeps them (WP-1675).

use kiln_ast::QualType;
use kiln_ast::attr::{AttrKind, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprDeps, ExprKind, GenericControl};
use kiln_ast::ids::{AttrId, DeclId, ExprId, ListId, RawId, StringLitId};
use kiln_ast::types::TypeView;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::attr_triage::Support;
use crate::decl::{AttrArg, sym_str};

/// Attributes GCC knows that clang does not: probing one is not an error
/// (GCC answers from its own storage; kiln has none, so 0).
const GCC_ONLY_ATTRS: &[&str] = &[
    "access",
    "copy",
    "externally_visible",
    "no_icf",
    "no_reorder",
    "noinit",
    "noipa",
    "nonstring",
    "optimize",
    "persistent",
    "simd",
    "symver",
    "tainted_args",
    "warn_if_not_aligned",
];

impl Sema<'_, '_> {
    /// `__builtin_has_attribute (op, name args)` at `loc`, closed at
    /// `rparen`. `op` is the first operand (a type-id or an unevaluated
    /// expression) and the attributes written in a type-id's
    /// decl-specifier-seq; `name` / `args` the attribute as written at
    /// `name_loc`.
    pub fn act_on_builtin_has_attribute(
        &mut self,
        loc: Loc,
        (op, written): (GenericControl, &[AttrId]),
        name: Symbol,
        name_loc: Loc,
        args: &[AttrArg],
        rparen: Loc,
    ) -> ExprId {
        let _ = rparen;
        match op {
            GenericControl::Expr(e) if self.is_err_expr(e) => return self.recover_err(&[e], loc),
            GenericControl::Type(t, _) if self.is_error_type(t) => {
                return self.recover_err(&[], loc);
            }
            _ => {}
        }
        let dependent = match op {
            GenericControl::Expr(e) => {
                self.ast.exprs.get(e).deps().intersects(ExprDeps::TYPE_VALUE_INST)
            }
            GenericControl::Type(t, _) => self.is_dependent_type(t),
        } || args.iter().any(|a| match *a {
            AttrArg::Expr(e) => self.ast.exprs.get(e).deps().intersects(ExprDeps::TYPE_VALUE_INST),
            AttrArg::Type(t) => self.is_dependent_type(t),
            AttrArg::Ident(..) => false,
        });
        // GCC 16.1 also refuses a class member access in a template
        // definition, dependent or not (its own tests expect that).
        let member_in_template = (self.in_template() || self.is_dependent_decl_context(self.ctx))
            && matches!(op, GenericControl::Expr(e)
                if self.ast.exprs.get(self.ignore_parens(e)).kind() == ExprKind::MemberExpr);
        if dependent || member_in_template {
            // GCC 16 does not support this either ("sorry, unimplemented").
            self.error(DiagId::sema_builtin_has_attribute_dependent, loc);
            return self.recover_err(&[], loc);
        }
        let v = self.has_attribute_value(op, written, name, name_loc, args);
        self.builder().bool_literal(QualType::BOOL, v, loc)
    }

    fn has_attribute_value(
        &mut self,
        op: GenericControl,
        written: &[AttrId],
        name: Symbol,
        name_loc: Loc,
        args: &[AttrArg],
    ) -> bool {
        let raw = sym_str(self, name);
        let n = kiln_ast::attr::normalize_attr_name(&raw).to_owned();
        let x86 = self.abi.triple.arch == kiln_abi::target::Arch::X86_64;
        let cxx26 = self.std >= kiln_lex::Std::Cxx26;
        let support = crate::attr_triage::attribute_support(
            &n,
            AttrSyntax::Gnu,
            None,
            self.clang_compat,
            cxx26,
            x86,
        );
        let class_b =
            crate::class_b_attrs::kind_of(&n, AttrSyntax::Gnu, None, x86, self.clang_compat);
        let kind = match (class_b, support) {
            (Some((k, _)), _) | (None, Support::Implemented(k)) => k,
            (None, Support::Unknown) => {
                if !GCC_ONLY_ATTRS.contains(&n.as_str()) {
                    let d =
                        self.diag(DiagId::sema_builtin_has_attribute_unknown, name_loc).arg(raw);
                    self.emit(d);
                }
                return false;
            }
            // Accepted silently and kept only for this query (WP-1675).
            (None, Support::Ignored(_)) if n == "nonstring" => AttrKind::NONSTRING,
            (None, Support::Ignored(_)) if n == "nocommon" => AttrKind::NOCOMMON,
            // Parsed and dropped (or rejected when written): never stored.
            (None, Support::Ignored(_) | Support::Unsupported(_)) => return false,
        };
        // A bare name asks for the attribute with any arguments (GCC lets
        // it omit required ones: `alloc_size`, `alias`); written
        // arguments are checked as on a declaration.
        let probe = if args.is_empty() {
            None
        } else if kind == AttrKind::FORMAT && args.len() < 3 {
            match self.format_attribute_n(AttrSyntax::Gnu, args, name_loc, true) {
                Some(p) => Some(p),
                None => return false,
            }
        } else {
            match self.act_on_attribute(AttrSyntax::Gnu, None, name, args, name_loc) {
                Some(p) => Some(p),
                None => return false,
            }
        };
        let kind = probe.map_or(kind, |p| self.ast.attrs.get(p).kind);
        // GCC keeps the attributes written in a type-id operand on the
        // type it builds (`ATTR (aligned (8)) int`); kiln, as clang, drops
        // them from the type, so they are asked here first (WP-1667).
        if self.written_has_attr(written, kind, probe) {
            return true;
        }
        if kind == AttrKind::ALIGNED {
            return self.has_aligned(op, probe);
        }
        if kind == AttrKind::VECTOR_SIZE {
            return self.has_vector_size(op, probe);
        }
        match op {
            GenericControl::Type(t, _) => self.type_has_attr(t, kind, probe),
            GenericControl::Expr(e) => {
                if let Some(d) = self.has_attr_operand_decl(e) {
                    self.decl_has_attr(d, kind, probe)
                        || self
                            .ast
                            .decls
                            .get(d)
                            .ty()
                            .is_some_and(|t| self.type_has_attr(t, kind, probe))
                } else {
                    let t = self.ety(e);
                    self.type_has_attr(t, kind, probe)
                }
            }
        }
    }

    /// Whether one of `written`, the attributes of a type-id operand's
    /// decl-specifier-seq, is a `kind` that `probe` matches (`aligned`:
    /// the requested alignment).
    fn written_has_attr(
        &mut self,
        written: &[AttrId],
        kind: AttrKind,
        probe: Option<AttrId>,
    ) -> bool {
        written.iter().any(|&a| {
            if self.ast.attrs.get(a).kind != kind {
                return false;
            }
            match probe {
                Some(p) if kind == AttrKind::ALIGNED => {
                    let want = self.requested_align(p);
                    want.is_some() && self.requested_align(a) == want
                }
                _ => self.attr_matches(a, kind, probe),
            }
        })
    }

    /// The declaration an expression operand names: a name or a member
    /// access, through parentheses and implicit conversions.
    fn has_attr_operand_decl(&self, e: ExprId) -> Option<DeclId> {
        let mut e = e;
        for _ in 0..256 {
            let p = self.ignore_parens(e);
            let q = self.ignore_implicit(p);
            if q == e {
                break;
            }
            e = q;
        }
        match self.ast.exprs.get(e).kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => Some(self.ast.exprs.decl(e)),
            _ => None,
        }
    }

    /// The attributes of every declaration of `d`, latest first.
    fn has_attr_decl_attrs(&self, d: DeclId) -> Vec<AttrId> {
        let mut out = Vec::new();
        let mut cur = Some(self.ast.latest(d));
        let mut n = 0;
        while let Some(x) = cur {
            out.extend(self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(x)));
            n += 1;
            if n > 4096 {
                break;
            }
            cur = self.ast.prev(x);
        }
        out
    }

    fn decl_has_attr(&mut self, d: DeclId, kind: AttrKind, probe: Option<AttrId>) -> bool {
        // GCC puts a `nonnull` written on a pointer (or reference) to
        // function on the function type, which the walk does not reach:
        // the variable, member or typedef answers 0 (WP-1675).
        if kind == AttrKind::NONNULL
            && !self.ast.decls.get(d).kind().is_function()
            && self.ast.decls.get(d).ty().is_some_and(|t| self.ast.types.pointee(t).is_some())
        {
            return false;
        }
        let attrs = self.has_attr_decl_attrs(d);
        attrs.into_iter().any(|a| self.attr_matches(a, kind, probe))
    }

    /// Whether `t` (through its sugar, then its class or enumeration)
    /// carries `kind`.
    fn type_has_attr(&mut self, t: QualType, kind: AttrKind, probe: Option<AttrId>) -> bool {
        let mut t = t;
        for _ in 0..256 {
            match self.ast.types.view(t.ty()) {
                TypeView::Typedef { decl, .. } if self.decl_has_attr(decl, kind, probe) => {
                    return true;
                }
                TypeView::Record(d) | TypeView::Enum(d) => {
                    return self.decl_has_attr(d, kind, probe);
                }
                _ => {}
            }
            let next = self.ast.types.desugar_once(t);
            if next.ty() == t.ty() {
                return false;
            }
            t = next;
        }
        false
    }

    /// Whether the stored attribute `a` is a `kind` that `probe` (its
    /// arguments; `None`: any) matches.
    fn attr_matches(&mut self, a: AttrId, kind: AttrKind, probe: Option<AttrId>) -> bool {
        let s = *self.ast.attrs.get(a);
        if s.kind != kind {
            return false;
        }
        let Some(p) = probe else {
            // GCC: a bare `nonnull` is only the one naming every parameter.
            return kind != AttrKind::NONNULL
                || self.ast.lists.iter::<ExprId>(ListId::new(s.a)).len() == 0;
        };
        let p = *self.ast.attrs.get(p);
        match kind {
            AttrKind::CONSTRUCTOR | AttrKind::DESTRUCTOR => {
                // The probe as written: `a` the argument's `ExprId`.
                let want = ExprId::from_raw(p.a).and_then(|e| self.fold_int(e)).unwrap_or(65535);
                i128::from(s.a) == want
            }
            AttrKind::ALLOC_SIZE => {
                let v = |s: &mut Self, w: u32| ExprId::from_raw(w).and_then(|e| s.fold_int(e));
                let (sa, sb, pa, pb) = (v(self, s.a), v(self, s.b), v(self, p.a), v(self, p.b));
                // GCC: the probe's positions are a subset of the stored ones.
                pa.is_some_and(|x| Some(x) == sa || Some(x) == sb)
                    && pb.is_none_or(|x| Some(x) == sa || Some(x) == sb)
            }
            AttrKind::ALLOC_ALIGN => {
                let v = |s: &mut Self, w: u32| ExprId::from_raw(w).and_then(|e| s.fold_int(e));
                v(self, s.a) == v(self, p.a)
            }
            AttrKind::NONNULL => {
                // GCC: a stored `nonnull` without positions covers every
                // probe; otherwise every probed position is stored.
                let list = |s: &mut Self, w: u32| -> Vec<Option<i128>> {
                    let ids: Vec<ExprId> = s.ast.lists.iter::<ExprId>(ListId::new(w)).collect();
                    ids.into_iter().map(|e| s.fold_int(e)).collect()
                };
                let stored = list(self, s.a);
                let probed = list(self, p.a);
                stored.is_empty()
                    || (!probed.is_empty() && probed.iter().all(|x| stored.contains(x)))
            }
            AttrKind::VISIBILITY => {
                // `internal` is stored as hidden with `b` = 1: GCC tells
                // the two apart (WP-1667).
                let vis = |s: &mut Self, n: kiln_ast::attr::AttrNode| {
                    if n.a == crate::visibility::UNCHECKED {
                        let internal = s.visibility_is_internal(n);
                        s.visibility_value(DeclKind::Function, n).map(|v| (v, internal))
                    } else {
                        Some((n.a, n.b == 1))
                    }
                };
                let want = vis(self, p);
                want.is_some() && vis(self, s) == want
            }
            AttrKind::CLEANUP => {
                // The probe as written: `a` the argument's `ExprId`; stored:
                // the function's `DeclId`.
                ExprId::from_raw(p.a)
                    .and_then(|e| self.has_attr_operand_decl(e))
                    .is_some_and(|d| d.raw() == s.a)
            }
            AttrKind::FORMAT => {
                // The archetype, then as many indices as the probe gives
                // (`flags`: 1 or 2 given; 0: all three).
                let n = if p.flags == 0 { 3 } else { p.flags };
                s.a == p.a
                    && (n < 2 || s.b & 0xffff == p.b & 0xffff)
                    && (n < 3 || s.b >> 16 == p.b >> 16)
            }
            AttrKind::SECTION | AttrKind::ALIAS => {
                let str_of = |w: u32| StringLitId::from_raw(w).map(|i| self.ast.strings.bytes(i));
                str_of(s.a).is_some() && str_of(s.a) == str_of(p.a)
            }
            _ => s.a == p.a && s.b == p.b,
        }
    }

    /// `aligned` / `aligned(N)` (GCC: the alignment was requested by the
    /// user, and, with `N`, is `N`).
    fn has_aligned(&mut self, op: GenericControl, probe: Option<AttrId>) -> bool {
        let want = match probe {
            Some(p) => match self.requested_align(p) {
                Some(v) => Some(v),
                None => return false,
            },
            None => None,
        };
        let t = match op {
            GenericControl::Type(t, _) => t,
            GenericControl::Expr(e) => {
                if let Some(d) = self.has_attr_operand_decl(e) {
                    let attrs = self.has_attr_decl_attrs(d);
                    let mut max = None::<u64>;
                    for a in attrs {
                        if self.ast.attrs.get(a).kind == AttrKind::ALIGNED {
                            let v = self.requested_align(a).unwrap_or(0);
                            max = Some(max.map_or(v, |m| m.max(v)));
                        }
                    }
                    let ty = self.ast.decls.get(d).ty();
                    if let Some(m) = max {
                        let natural = ty.map_or(0, |t| self.has_attr_type_align(t));
                        return want.is_none_or(|w| w == m.max(natural));
                    }
                    match ty {
                        Some(t) => t,
                        None => return false,
                    }
                } else {
                    self.ety(e)
                }
            }
        };
        if !self.type_user_aligned(t) {
            return false;
        }
        want.is_none_or(|w| w == self.has_attr_type_align(t))
    }

    /// `vector_size` / `vector_size(N)`: consumed into the declared
    /// type, so asked of the (declaration's) type: a GNU vector, of `N`
    /// bytes.
    fn has_vector_size(&mut self, op: GenericControl, probe: Option<AttrId>) -> bool {
        let t = match op {
            GenericControl::Type(t, _) => t,
            GenericControl::Expr(e) => match self.has_attr_operand_decl(e) {
                Some(d) => match self.ast.decls.get(d).ty() {
                    Some(t) => t,
                    None => return false,
                },
                None => self.ety(e),
            },
        };
        let t = self.ast.types.canonical(t);
        if !matches!(self.ast.types.view(t.ty()), TypeView::Vector { .. }) {
            return false;
        }
        let Some(p) = probe else { return true };
        let want = ExprId::from_raw(self.ast.attrs.get(p).a).and_then(|e| self.fold_int(e));
        let size = crate::target::size_align(&self.ast, t, &self.abi).map(|x| i128::from(x.0));
        want.is_some() && want == size
    }

    fn has_attr_type_align(&self, t: QualType) -> u64 {
        crate::target::size_align(&self.ast, t, &self.abi).map_or(0, |x| x.1)
    }

    /// Whether `t`'s alignment was requested by an `aligned` attribute on
    /// a typedef it was named by or on its class or enumeration; an array
    /// has its element type's (GCC's `TYPE_USER_ALIGN` propagation).
    fn type_user_aligned(&mut self, t: QualType) -> bool {
        let mut t = t;
        for _ in 0..256 {
            match self.ast.types.view(t.ty()) {
                TypeView::Typedef { decl, .. }
                    if self.decl_has_attr(decl, AttrKind::ALIGNED, None) =>
                {
                    return true;
                }
                TypeView::Record(d) | TypeView::Enum(d) => {
                    return self.decl_has_attr(d, AttrKind::ALIGNED, None);
                }
                TypeView::ConstantArray(a) => {
                    t = a.elem;
                    continue;
                }
                TypeView::IncompleteArray(e) => {
                    t = e;
                    continue;
                }
                _ => {}
            }
            let next = self.ast.types.desugar_once(t);
            if next.ty() == t.ty() {
                return false;
            }
            t = next;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::GCC_ONLY_ATTRS;

    #[test]
    fn gcc_only_attrs_sorted() {
        assert!(GCC_ONLY_ATTRS.windows(2).all(|w| w[0] < w[1]));
    }
}
