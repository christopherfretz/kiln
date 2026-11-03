//! GNU `nonnull` and `returns_nonnull` (clang `NonNullAttr` /
//! `ReturnsNonNullAttr`; spec/sema.md §5.27): the argument checks of
//! clang's `handleNonNullAttr` / `handleReturnsNonNullAttr`, and the
//! queries the always-non-null warnings ask (`null_compare.rs`).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprDeps, Radix};
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId, ListId, QualType};
use kiln_diag::DiagId;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::decl::AttrArg;

impl Sema<'_, '_> {
    /// `nonnull(n, ...)` as written: integer constants, each kept as an
    /// `IntegerLiteral` at its argument (zero for a negative value) and
    /// checked against the declaration in `check_decl_attrs`
    /// ([`Sema::check_nonnull_attr`]). A value-dependent index drops the
    /// attribute.
    pub(crate) fn nonnull_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let mut idx = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let &AttrArg::Expr(e) = arg else {
                let msg = self
                    .diag(DiagId::sema_attribute_param_not_ice, loc)
                    .arg("'nonnull'")
                    .arg((i + 1).to_string());
                self.emit(msg);
                return None;
            };
            // A value-dependent index is not an integer constant either
            // (clang checks the attribute on the template pattern).
            let dependent =
                self.ast.exprs.get(e).deps().intersects(ExprDeps::VALUE | ExprDeps::TYPE);
            let Some(v) = (!dependent).then(|| self.fold_int(e)).flatten() else {
                let msg = self
                    .diag(DiagId::sema_attribute_param_not_ice, loc)
                    .arg("'nonnull'")
                    .arg((i + 1).to_string())
                    .range(self.expr_range(e));
                self.emit(msg);
                return None;
            };
            let v = u64::try_from(v).unwrap_or(0);
            let ty = self.size_type();
            let at = self.ast.exprs.get(e).loc();
            idx.push(self.builder().int_literal(ty, v, Radix::Decimal, at));
        }
        let list = self.ast.lists.push(&idx);
        let node = AttrNode { kind: AttrKind::NONNULL, syntax, flags: 0, loc, a: list.raw(), b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// clang `handleNonNullAttr` for `nonnull` attribute `a` on
    /// declaration `d` of kind `kind`: a function (or method); each
    /// index names a parameter (one-based; 1 is `this` in an implicit
    /// object member function, which is an error), a variadic function
    /// takes any positive index. An index naming a non-pointer parameter
    /// is dropped with a warning; when every index is dropped the
    /// attribute names every parameter, as in clang. Without indices, a
    /// function with no pointer parameter (and not variadic) is warned.
    /// False drops the attribute.
    pub(crate) fn check_nonnull_attr(
        &mut self,
        d: DeclId,
        kind: DeclKind,
        a: AttrId,
        node: AttrNode,
    ) -> bool {
        let shown = format!("'{}'", self.attr_shown(&node, "nonnull"));
        if kind == DeclKind::ParmVar {
            return true;
        }
        let proto = self.ast.decls.get(d).ty().and_then(|t| self.ast.types.proto(t));
        let Some(proto) = proto.filter(|_| kind.is_function()) else {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(shown)
                .arg("functions, methods, and parameters");
            self.emit(msg);
            return false;
        };
        let params = self.ast.types.params(proto).to_vec();
        let variadic = proto.variadic();
        let this = u64::from(self.is_implicit_object_member(d));
        let written = ListId::from_raw(node.a).unwrap_or(ListId::EMPTY);
        let written: Vec<ExprId> = self.ast.lists.iter::<ExprId>(written).collect();
        let mut kept = Vec::with_capacity(written.len());
        for (i, &e) in written.iter().enumerate() {
            let n = self.ast.exprs.int_value(e).unwrap_or(0);
            let range = self.expr_range(e);
            if this == 1 && n == 1 {
                let msg = self
                    .diag(DiagId::sema_attribute_implicit_this_arg, node.loc)
                    .arg(shown)
                    .range(range);
                self.emit(msg);
                return false;
            }
            let k = n.checked_sub(1 + this).and_then(|k| usize::try_from(k).ok());
            let p = k.and_then(|k| params.get(k).copied());
            if n == 0 || (p.is_none() && !variadic) {
                let msg = self
                    .diag(DiagId::sema_attribute_param_out_of_bounds, node.loc)
                    .arg(shown)
                    .arg((i + 1).to_string())
                    .range(range);
                self.emit(msg);
                return false;
            }
            if let Some(p) = p
                && !self.nonnull_pointer_type(p)
            {
                let msg = self
                    .diag(DiagId::sema_attribute_pointer_args_only, node.loc)
                    .arg(shown.clone())
                    .range(range);
                self.emit(msg);
                continue;
            }
            kept.push(e);
        }
        if written.is_empty()
            && !node.loc.is_macro()
            && !variadic
            && !params
                .iter()
                .any(|&p| self.ast.types.is_dependent(p) || self.nonnull_pointer_type(p))
        {
            let msg = self.diag(DiagId::sema_attribute_nonnull_no_pointers, node.loc);
            self.emit(msg);
        }
        let list = self.ast.lists.push(&kept);
        self.ast.attrs.set_args(a, list.raw(), 0);
        true
    }

    /// clang `handleReturnsNonNullAttr`: on a function returning a
    /// pointer (or a dependent type).
    pub(crate) fn check_returns_nonnull_attr(
        &mut self,
        d: DeclId,
        kind: DeclKind,
        node: AttrNode,
    ) -> bool {
        let shown = format!("'{}'", self.attr_shown(&node, "returns_nonnull"));
        let proto = self.ast.decls.get(d).ty().and_then(|t| self.ast.types.proto(t));
        let Some(ret) = proto.filter(|_| kind.is_function()).map(kiln_ast::types::FuncProto::ret)
        else {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(shown)
                .arg("Objective-C methods and functions");
            self.emit(msg);
            return false;
        };
        if !self.nonnull_pointer_type(ret) {
            let msg = self.diag(DiagId::sema_attribute_pointers_only, node.loc).arg(shown);
            self.emit(msg);
            return false;
        }
        true
    }

    /// clang `isValidPointerAttrType` without references: a pointer
    /// (through a reference), or a dependent type from clang 22 on
    /// (clang 19 warns on a template parameter type).
    fn nonnull_pointer_type(&self, t: QualType) -> bool {
        let t = self.non_reference(t);
        (self.clang_compat >= 22 && self.ast.types.is_dependent(t)) || self.ast.types.is_pointer(t)
    }

    /// `nonnull` parameter attributes (`void f(__attribute__((nonnull))
    /// int *p)`) on a parameter of non-pointer type are dropped with a
    /// warning underlining the parameter-declaration `range` (clang
    /// `handleNonNullAttrParameter`).
    pub(crate) fn check_param_nonnull_attrs(
        &mut self,
        ty: QualType,
        range: Span,
        attrs: &mut Vec<AttrId>,
    ) {
        if !attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::NONNULL) {
            return;
        }
        let pointer = self.nonnull_pointer_type(ty);
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::NONNULL || pointer {
                return true;
            }
            let shown = format!("'{}'", self.attr_shown(&node, "nonnull"));
            let msg = self
                .diag(DiagId::sema_attribute_pointer_args_only, node.loc)
                .arg(shown)
                .range(range);
            self.emit(msg);
            false
        });
    }

    /// The `nonnull` attribute that makes parameter `p` non-null, with
    /// whether it is the parameter's own (clang
    /// `DiagnoseAlwaysNonNullPointer`): the parameter's own, on any
    /// declaration of its function; else one on a declaration of the
    /// function naming every parameter or `p`'s index. A function
    /// template's pattern has none (clang skips it until specialized).
    pub(crate) fn nonnull_param_attr(&self, p: DeclId) -> Option<(AttrNode, bool)> {
        let index = self.ast.decls.param(p)?.index;
        let f = self.ast.decls.get(p).ctx().filter(|&f| self.ast.decls.function(f).is_some())?;
        let redecls = |s: &Self| {
            let mut v = Vec::new();
            let mut cur = Some(f);
            while let Some(x) = cur
                && v.len() < 4096
            {
                v.push(x);
                cur = s.ast.prev(x);
            }
            v
        };
        let chain = redecls(self);
        for &g in &chain {
            let q = self.param_decls(g).get(usize::from(index)).copied();
            if let Some(a) = q.and_then(|q| self.find_attr(q, AttrKind::NONNULL)) {
                return Some((*self.ast.attrs.get(a), true));
            }
        }
        if self.ast.described_template(f).is_some() {
            return None;
        }
        let this = u64::from(self.is_implicit_object_member(f));
        let want = u64::from(index) + 1 + this;
        for &g in &chain {
            for a in self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(g)) {
                let node = self.ast.attrs.get(a);
                if node.kind != AttrKind::NONNULL {
                    continue;
                }
                let list = ListId::from_raw(node.a).unwrap_or(ListId::EMPTY);
                let mut idx = self.ast.lists.iter::<ExprId>(list).peekable();
                if idx.peek().is_none()
                    || idx.any(|e| self.ast.exprs.int_value(e).is_some_and(|n| n == want))
                {
                    return Some((*node, false));
                }
            }
        }
        None
    }

    /// Whether `d`'s parameter `p` is assigned or has its address taken
    /// earlier in the current function (clang `ModifiedNonNullParams`).
    pub(crate) fn nonnull_param_modified(&self, p: DeclId) -> bool {
        self.func.as_ref().is_none_or(|f| f.modified_params.contains(&p))
    }

    /// Records that parameter `e` (a `DeclRefExpr`) is assigned or has
    /// its address taken (clang `RecordModifiableNonNullParam`).
    pub(crate) fn record_modified_param(&mut self, e: ExprId) {
        if self.ast.exprs.get(e).kind() != kiln_ast::expr::ExprKind::DeclRefExpr {
            return;
        }
        let d = self.ast.exprs.decl(e);
        if self.ast.decls.get(d).kind() != DeclKind::ParmVar {
            return;
        }
        if let Some(f) = self.func.as_mut()
            && !f.modified_params.contains(&d)
        {
            f.modified_params.push(d);
        }
    }
}
