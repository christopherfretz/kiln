//! Address spaces (spec/sema.md "Address spaces", spec/types.md §5;
//! WP-1803): clang's `address_space(N)` and `opencl_*` type attributes in
//! C++ (`HandleAddressSpaceTypeAttribute`). The attribute is consumed into
//! the declared type as an `ExtQual` qualifier, like the vector attributes
//! (`vector.rs`), never attached to the declaration. Conversions between
//! spaces are in `conv/`, the mangling in `kiln_abi`, the IR in
//! `kiln_codegen`.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclFlags, TlsKind, VarStorage};
use kiln_ast::ids::RawId;
use kiln_ast::types::AddrSpace;
use kiln_ast::{AttrId, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::decl::AttrArg;

/// clang `Qualifiers::MaxAddressSpace` (`0x7fffff`) less clang's
/// `LangAS::FirstTargetAddressSpace` (26 at clang 22, 21 at clang 19):
/// the largest `N` of `address_space(N)`.
fn max_target_space(compat: u32) -> u32 {
    0x7f_ffff - if compat >= 22 { 26 } else { 21 }
}

impl Sema<'_, '_> {
    /// `address_space(N)` or an `opencl_*` attribute `name` as written
    /// (`shown` for diagnostics): the node of the space, `a` its
    /// [`AddrSpace`] word; `None` after a diagnostic (clang
    /// `HandleAddressSpaceTypeAttribute` drops the attribute and keeps
    /// the type).
    pub(crate) fn address_space_attribute(
        &mut self,
        name: &str,
        syntax: AttrSyntax,
        shown: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let space = if let Some(s) = AddrSpace::opencl(name) {
            if !args.is_empty() {
                let msg =
                    self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(shown).arg("0");
                self.emit(msg);
                return None;
            }
            s
        } else {
            let [AttrArg::Expr(e)] = args else {
                let msg =
                    self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(shown).arg("1");
                self.emit(msg);
                return None;
            };
            // A value-dependent `N` (clang `BuildAddressSpaceIndex` checks
            // nothing yet): the node keeps the expression, and the type is
            // a `DependentAddrSpace` substituted at instantiation
            // (WP-1811), diagnosed at this attribute.
            if self.expr_is_value_dependent(*e) {
                if self.is_error_type(self.ast.exprs.get(*e).ty()) {
                    return None;
                }
                self.inst.type_attr_locs.insert(*e, loc);
                let node = AttrNode {
                    kind: AttrKind::ADDRESS_SPACE,
                    syntax,
                    flags: AttrNode::SPACE_DEPENDENT,
                    loc,
                    a: (*e).raw(),
                    b: 0,
                };
                return Some(self.ast.attrs.push(node));
            }
            self.address_space_number(*e, shown, loc)?
        };
        let node =
            AttrNode { kind: AttrKind::ADDRESS_SPACE, syntax, flags: 0, loc, a: space.0, b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// The space `address_space(e)` names: `e` must be an integral
    /// constant expression in `0..=max` (clang's `BuildAddressSpaceAttr`
    /// wording). Also the substituted operand of a `DependentAddrSpace`
    /// at instantiation, where `shown` is `address_space`.
    pub(crate) fn address_space_number(
        &mut self,
        e: kiln_ast::ExprId,
        shown: &str,
        loc: Loc,
    ) -> Option<AddrSpace> {
        let ty = self.ast.exprs.get(e).ty();
        if self.is_error_type(ty) {
            return None;
        }
        let quoted = format!("'{shown}'");
        if self.expr_is_value_dependent(e) || !self.is_integral_or_enum(ty) {
            let msg = self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(quoted);
            self.emit(msg);
            return None;
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_full_expr(c);
        let Ok(v) = self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e))) else {
            let msg = self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(quoted);
            self.emit(msg);
            return None;
        };
        if v < 0 {
            self.error(DiagId::sema_address_space_negative, loc);
            return None;
        }
        let max = max_target_space(self.clang_compat);
        match u32::try_from(v) {
            Ok(n) if n <= max => Some(AddrSpace::target(n)),
            _ => {
                let msg = self.diag(DiagId::sema_address_space_too_high, loc).arg(max.to_string());
                self.emit(msg);
                None
            }
        }
    }

    /// The address-space attributes after one `*`, folded into the
    /// first (a second space is diagnosed as on any type).
    pub fn fold_space_attrs(&mut self, spaces: &[AttrId]) -> Option<AttrId> {
        let (&first, rest) = spaces.split_first()?;
        let f = *self.ast.attrs.get(first);
        for &r in rest {
            let n = *self.ast.attrs.get(r);
            // A dependent one is never the same space (clang diagnoses a
            // second space over a `DependentAddressSpaceType`).
            let dependent = (n.flags | f.flags) & AttrNode::SPACE_DEPENDENT != 0;
            let id = if n.a == f.a && !dependent {
                DiagId::sema_address_space_multiple_identical
            } else {
                DiagId::sema_address_space_multiple
            };
            self.error(id, n.loc);
        }
        Some(first)
    }

    /// `ty` qualified with the space of `node` (clang
    /// `HandleAddressSpaceTypeAttribute`): not on a function type; a
    /// second, different space is an error, the same one again a
    /// warning; on an array type the elements are qualified
    /// ([basic.type.qualifier]/3, as cv). A reference type is left alone.
    ///
    /// A value-dependent space ([`AttrNode::SPACE_DEPENDENT`]) makes a
    /// `DependentAddrSpace` over `ty` (clang `BuildAddressSpaceAttr`):
    /// over another one it is "multiple address spaces specified for
    /// type"; over a type already in a space the conflict is diagnosed at
    /// instantiation ([`Self::subst_addr_space`]).
    pub(crate) fn build_addr_space_type(&mut self, ty: QualType, node: AttrNode) -> QualType {
        if self.is_error_type(ty) {
            return ty;
        }
        let t = &self.ast.types;
        if t.is_function(ty) {
            self.error(DiagId::sema_address_space_function_type, node.loc);
            return ty;
        }
        if node.flags & AttrNode::SPACE_DEPENDENT != 0 {
            if t.is_dependent_addr_space(ty) {
                self.error(DiagId::sema_address_space_multiple, node.loc);
                return ty;
            }
            let Some(e) = kiln_ast::ExprId::from_raw(node.a) else { return ty };
            let p = self.dependent_profile(e);
            return self.ast.types.dependent_addr_space(ty, e, p);
        }
        let space = AddrSpace(node.a);
        if t.is_reference(ty) {
            return ty;
        }
        let old = t.addr_space(ty);
        if old != AddrSpace::DEFAULT {
            if old == space {
                self.error(DiagId::sema_address_space_multiple_identical, node.loc);
            } else {
                self.error(DiagId::sema_address_space_multiple, node.loc);
            }
            return ty;
        }
        self.ast.types.with_addr_space(ty, space)
    }

    /// [temp.inst] of `pointee __attribute__((address_space(e)))`, `e`
    /// and `pointee` substituted, the attribute at `loc` (clang
    /// `RebuildDependentAddressSpaceType` → `BuildAddressSpaceAttr`): a
    /// still value-dependent `e` (a partial substitution) keeps the
    /// dependent form; else the space is checked as written (`'address_space'
    /// attribute requires an integer constant`, negative, too large) and
    /// qualifies `pointee`, a different space already there an error, the
    /// same one a warning. `None` after an error.
    pub(crate) fn subst_addr_space(
        &mut self,
        pointee: QualType,
        e: kiln_ast::ExprId,
        loc: Loc,
    ) -> Option<QualType> {
        if self.is_error_type(pointee) {
            return None;
        }
        if self.expr_is_value_dependent(e) {
            self.inst.type_attr_locs.insert(e, loc);
            let p = self.dependent_profile(e);
            return Some(self.ast.types.dependent_addr_space(pointee, e, p));
        }
        let space = self.address_space_number(e, "address_space", loc)?;
        let t = &self.ast.types;
        if t.is_reference(pointee) {
            return Some(pointee);
        }
        let old = t.addr_space(pointee);
        if old != AddrSpace::DEFAULT {
            if old == space {
                self.error(DiagId::sema_address_space_multiple_identical, loc);
            } else {
                self.error(DiagId::sema_address_space_multiple, loc);
                return None;
            }
        }
        Some(self.ast.types.with_addr_space(pointee, space))
    }

    /// A variable `v` whose type is in an address space: an error when it
    /// has automatic storage duration (clang `CheckVariableDeclarationType`
    /// "automatic variable qualified with an address space"); `v` is then
    /// invalid.
    pub(crate) fn check_addr_space_var(&mut self, v: DeclId, local: bool) {
        let n = *self.ast.decls.get(v);
        let Some(ty) = n.ty() else { return };
        if !local || self.ast.types.addr_space(ty) == AddrSpace::DEFAULT {
            return;
        }
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        if matches!(spec.storage(), VarStorage::Static | VarStorage::Extern)
            || spec.tls() != TlsKind::None
        {
            return;
        }
        self.error(DiagId::sema_address_space_automatic, n.loc());
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
    }

    /// Whether `ty`, the adjusted type of a parameter declared at `loc`,
    /// is in an address space: clang's "parameter may not be qualified
    /// with an address space" (the parameter is invalid).
    pub(crate) fn check_addr_space_param(&mut self, ty: QualType, loc: Loc) -> bool {
        if self.ast.types.addr_space(ty) == AddrSpace::DEFAULT || self.is_error_type(ty) {
            return false;
        }
        self.error(DiagId::sema_address_space_parameter, loc);
        true
    }
}
