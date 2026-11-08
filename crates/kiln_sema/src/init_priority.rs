//! GNU `init_priority(N)` (spec/sema.md §4.5 `init_priority`; clang
//! `handleInitPriorityAttr`): the priority of a namespace-scope object's
//! dynamic initialization ([basic.start.dynamic]/1 leaves the order of
//! initializations across translation units unspecified; the attribute
//! orders them through the `@llvm.global_ctors` priority).
//!
//! `act_on_attribute` keeps the argument expression as written (in `a`);
//! [`Sema::check_init_priority`] checks the subject and the value in
//! clang's order and replaces the node with one holding the priority.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

impl Sema<'_, '_> {
    /// `init_priority(N)` as written: exactly one argument (clang's
    /// generic argument-count check), kept unchecked in `a`.
    pub(crate) fn init_priority_attribute(
        &mut self,
        syntax: AttrSyntax,
        shown: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let [AttrArg::Expr(e)] = args else {
            // clang 22 names a `[[gnu::x]]` attribute with its scope.
            let name = if syntax == AttrSyntax::Cxx11 && self.clang_compat >= 22 {
                shown.to_owned()
            } else {
                "init_priority".to_owned()
            };
            let msg = self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(name).arg("1");
            self.emit(msg);
            return None;
        };
        let node =
            AttrNode { kind: AttrKind::INIT_PRIORITY, syntax, flags: 0, loc, a: e.raw(), b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// Checks the `init_priority` attributes among `attrs` of declaration
    /// `d`, dropping the invalid ones and replacing each valid one by a
    /// node whose `a` is the priority.
    pub(crate) fn check_init_priority(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if node.kind != AttrKind::INIT_PRIORITY {
                i += 1;
                continue;
            }
            match self.init_priority_value(d, node) {
                Some(p) => {
                    attrs[i] = self.ast.attrs.push(AttrNode { a: p, ..node });
                    i += 1;
                }
                None => {
                    attrs.remove(i);
                }
            }
        }
    }

    /// The priority of `init_priority` attribute `node` on `d`, or `None`
    /// after a diagnostic.
    fn init_priority_value(&mut self, d: DeclId, node: AttrNode) -> Option<u32> {
        let shown = self.attr_shown(&node, "init_priority");
        let name = format!("'{shown}'");
        if !matches!(self.ast.decls.get(d).kind(), DeclKind::Var) {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                .arg(name)
                .arg("variables");
            self.emit(msg);
            return None;
        }
        // Not a block-scope object, and of class type (or an array of
        // it).
        let in_function = self.ast.decls.get(d).ctx().is_some_and(|c| self.in_function_ctx(c));
        let mut ty = self.ast.decls.get(d).ty();
        while let Some(t) = ty.filter(|&t| self.ast.types.is_array(t)) {
            ty = self.ast.types.element(t);
        }
        if in_function || ty.is_none_or(|t| self.ast.types.as_record(t).is_none()) {
            self.error(DiagId::sema_init_priority_object_attr, node.loc);
            return None;
        }
        let e = ExprId::from_raw(node.a)?;
        let p = self.uint32_attr_arg(e, &name, node.loc)?;
        if self.clang_compat >= 22 {
            if p > 65535 {
                let msg = self
                    .diag(DiagId::sema_attribute_argument_out_of_range, node.loc)
                    .arg(name)
                    .arg("0")
                    .arg("65535");
                self.emit(msg);
                return None;
            }
            // Values up to 100 are reserved for the implementation.
            if p < 101 {
                let msg =
                    self.diag(DiagId::sema_init_priority_reserved, node.loc).arg(p.to_string());
                self.emit(msg);
            }
        } else if !(101..=65535).contains(&p) && !self.diags.sm().is_in_system_header(node.loc) {
            let msg = self
                .diag(DiagId::sema_attribute_argument_out_of_range, node.loc)
                .arg(name)
                .arg("101")
                .arg("65535");
            self.emit(msg);
            return None;
        }
        Some(p)
    }

    /// clang `checkUInt32Argument` of attribute argument `e` of the
    /// attribute shown as `name` (quoted) at `loc`: its value, or `None`
    /// after a diagnostic.
    pub(crate) fn uint32_attr_arg(&mut self, e: ExprId, name: &str, loc: Loc) -> Option<u32> {
        let at = self.ast.exprs.get(e).loc();
        let Some(v) = self.fold_int(e) else {
            let msg = self
                .diag(DiagId::sema_attribute_requires_integer_constant, loc)
                .arg(name.to_owned());
            self.emit(msg);
            return None;
        };
        let width = self.ast.exprs.get(e).ty();
        let width = crate::conv::rank::int_width(self, width).unwrap_or(32);
        // `APSInt::isIntN(32)`: the value's active bits in its own width.
        let bits = if v < 0 { i128::from(width) } else { i128::from(128 - v.leading_zeros()) };
        if bits > 32 {
            let msg =
                self.diag(DiagId::sema_ice_too_large_unsigned, at).arg(v.to_string()).arg("32");
            self.emit(msg);
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(v as u32)
    }

    /// True if context `c` is (inside) a function.
    fn in_function_ctx(&self, c: DeclId) -> bool {
        let mut cur = Some(c);
        let mut guard = 0;
        while let Some(d) = cur {
            let k = self.ast.decls.get(d).kind();
            if k.is_function() {
                return true;
            }
            if matches!(k, DeclKind::TranslationUnit | DeclKind::Namespace) || guard > 1024 {
                return false;
            }
            guard += 1;
            cur = self.ast.decls.get(d).ctx();
        }
        false
    }
}
