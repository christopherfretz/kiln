//! Vector types (spec/types.md §1 `Vector` / `DependentVector`, spec/sema.md
//! §5.8): clang's `ext_vector_type(N)` and GNU `vector_size(N)` attributes
//! on a typedef or alias-declaration (libc++ `__algorithm/simd_utils.h`
//! `using __simd_vector __attribute__((__ext_vector_type__(_Np))) = _Tp;`).
//!
//! Both attributes are consumed into the declared type and never attached
//! to the declaration (clang's AST keeps only the type). A value-dependent
//! size, or a dependent element of a `vector_size` vector, gives a
//! `DependentVector` whose operand is the attribute's expression as
//! written (the element count for `ext_vector_type`, the byte size for
//! `vector_size`, as clang's `DependentVectorType` /
//! `DependentSizedExtVectorType`); substitution rebuilds it through
//! [`Sema::build_vector_type`]. Among the decl-specifiers the attributes
//! modify the decl-specifier type (`__attribute__((vector_size(8))) short
//! f();` returns a vector); after a declarator they modify the whole
//! declared type, and an invalid one leaves the type unchanged (clang
//! `processTypeAttrs`, `HandleVectorSizeAttr`). Operations on vector
//! values are in `vector_ops.rs` (WP-156); their constant evaluation is M4.

use std::ops::ControlFlow;

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::ids::RawId;
use kiln_ast::types::{TypeView, VectorKind};
use kiln_ast::{AttrId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::decl::AttrArg;
use crate::declspec::DeclSpec;

impl Sema<'_, '_> {
    /// `ext_vector_type(N)` / `vector_size(N)` as written: exactly one
    /// expression argument, kept unevaluated until the attribute is
    /// applied to its declaration.
    pub(crate) fn vector_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        raw: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let n = raw.trim_start_matches("__").trim_end_matches("__").to_owned();
        let e = match args {
            [AttrArg::Expr(e)] => *e,
            [AttrArg::Type(_)] => {
                self.error(DiagId::parse_expected_expression, loc);
                return None;
            }
            _ => {
                // clang 22 checks `ext_vector_type`'s arguments as a
                // declaration attribute and again as a type attribute:
                // the error twice (spec/sema.md §1.1).
                let times = if kind == AttrKind::EXT_VECTOR_TYPE && self.clang_compat >= 22 {
                    2
                } else {
                    1
                };
                for _ in 0..times {
                    let msg = self
                        .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                        .arg(n.clone())
                        .arg("1");
                    self.emit(if times == 2 { msg.allow_duplicate() } else { msg });
                }
                return None;
            }
        };
        let node = AttrNode { kind, syntax, flags: 0, loc, a: e.raw(), b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// The attributes `act_on_attribute` hands off whole: the vector
    /// attributes and the GNU allocation attributes (`Break` with the
    /// result); `Continue` for any other kind.
    pub(crate) fn attribute_with_args(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        raw: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> ControlFlow<Option<AttrId>> {
        match kind {
            AttrKind::EXT_VECTOR_TYPE | AttrKind::VECTOR_SIZE => {
                ControlFlow::Break(self.vector_attribute(kind, syntax, raw, args, loc))
            }
            AttrKind::ALLOC_SIZE | AttrKind::ALLOC_ALIGN => {
                ControlFlow::Break(self.alloc_attribute(kind, syntax, args, loc))
            }
            AttrKind::FORMAT => ControlFlow::Break(self.format_attribute(syntax, args, loc)),
            _ => ControlFlow::Continue(()),
        }
    }

    /// GNU `format(archetype, F, A)` as written (WP-186; clang
    /// `handleFormatAttr`): exactly three arguments, an identifier then
    /// two integer constants. The archetype (`__x__` normalized to `x`)
    /// is stored in `a`: [`FORMAT_PRINTF`] (checked at calls),
    /// [`FORMAT_STRFTIME`] or [`FORMAT_OTHER`] (a format clang supports
    /// that kiln does not check); an unknown one is ignored with a
    /// warning, and GCC's internal `gcc_*diag` silently. `b` = `F | A <<
    /// 16`, each saturated at `0xfffe` (a negative value is out of
    /// bounds); the indices are checked against the declaration in
    /// `check_decl_attrs`.
    pub(crate) fn format_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let name = "'format'";
        if args.len() != 3 {
            let msg =
                self.diag(DiagId::sema_attribute_wrong_number_arguments, loc).arg(name).arg("3");
            self.emit(msg);
            return None;
        }
        let AttrArg::Ident(sym, _) = args[0] else {
            let msg = self.diag(DiagId::sema_attribute_param_not_ident, loc).arg(name).arg("1");
            self.emit(msg);
            return None;
        };
        let written = self.ast.symbols().as_str(sym).to_owned();
        let arch = match written.strip_prefix("__").and_then(|w| w.strip_suffix("__")) {
            Some(w) if written.len() >= 4 => w,
            _ => written.as_str(),
        };
        // clang 22 maps GCC's `gnu_printf` / `gnu_scanf` to the plain
        // archetypes; clang 19 does not know them.
        let gnu22 = self.clang_compat >= 22;
        let a = match arch {
            "printf" => FORMAT_PRINTF,
            "gnu_printf" if gnu22 => FORMAT_PRINTF,
            "strftime" => FORMAT_STRFTIME,
            "scanf" | "printf0" | "strfmon" | "cmn_err" | "vcmn_err" | "zcmn_err" | "kprintf"
            | "freebsd_kprintf" | "os_trace" | "os_log" | "NSString" | "CFString" => FORMAT_OTHER,
            "gnu_scanf" if gnu22 => FORMAT_OTHER,
            "gcc_diag" | "gcc_cdiag" | "gcc_cxxdiag" | "gcc_tdiag" => return None,
            _ => {
                let msg =
                    self.diag(DiagId::sema_attribute_arg_not_supported, loc).arg(name).arg(arch);
                self.emit(msg);
                return None;
            }
        };
        let mut idx = [0u32; 2];
        for (i, arg) in args[1..].iter().enumerate() {
            let v = match arg {
                AttrArg::Expr(e) => self.fold_int(*e),
                AttrArg::Type(_) | AttrArg::Ident(..) => None,
            };
            let Some(v) = v else {
                let msg = self
                    .diag(DiagId::sema_attribute_param_not_ice, loc)
                    .arg(name)
                    .arg((i + 2).to_string());
                self.emit(msg);
                return None;
            };
            idx[i] = u32::try_from(v).map_or(0xfffe, |v| v.min(0xfffe));
        }
        let node =
            AttrNode { kind: AttrKind::FORMAT, syntax, flags: 0, loc, a, b: idx[0] | idx[1] << 16 };
        Some(self.ast.attrs.push(node))
    }

    /// GNU `alloc_size(n[, m])` / `alloc_align(n)` as written: integer
    /// constants, each kept as an `IntegerLiteral` at its argument (zero
    /// for a negative value, which is out of bounds) and checked against
    /// the declaration in `check_decl_attrs` (clang `handleAllocSizeAttr`
    /// / `AddAllocAlignAttr`).
    pub(crate) fn alloc_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let size = kind == AttrKind::ALLOC_SIZE;
        let name = if size { "'alloc_size'" } else { "'alloc_align'" };
        if size && (args.is_empty() || args.len() > 2) {
            let msg =
                self.diag(DiagId::sema_alloc_size_arg_count, loc).arg(u32::from(!args.is_empty()));
            self.emit(msg);
            return None;
        }
        if !size && args.len() != 1 {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg("alloc_align").arg("1");
            self.emit(msg);
            return None;
        }
        let mut idx = [0u32; 2];
        for (i, arg) in args.iter().enumerate() {
            let v = match arg {
                AttrArg::Expr(e) => self.fold_int(*e).map(|v| (v, self.ast.exprs.get(*e).loc())),
                AttrArg::Type(_) | AttrArg::Ident(..) => None,
            };
            let Some((v, at)) = v else {
                let msg = self
                    .diag(DiagId::sema_attribute_param_not_ice, loc)
                    .arg(name)
                    .arg((i + 1).to_string());
                self.emit(msg);
                return None;
            };
            let v = u64::try_from(v).unwrap_or(0);
            let ty = self.size_type();
            idx[i] = self.builder().int_literal(ty, v, kiln_ast::expr::Radix::Decimal, at).raw();
        }
        let node = AttrNode { kind, syntax, flags: 0, loc, a: idx[0], b: idx[1] };
        Some(self.ast.attrs.push(node))
    }

    /// Applies (and removes from `attrs`) the vector attributes of a
    /// declaration of type `ty`. `typedef` is true for a typedef or
    /// alias-declaration, the only declarations `ext_vector_type` applies
    /// to at clang 19 (clang 22 takes any); `vector_size` also applies to
    /// the type of a variable, field or parameter (on a function clang applies it to the return type: not
    /// supported yet). Returns the declared type.
    pub(crate) fn apply_vector_attrs(
        &mut self,
        ty: QualType,
        attrs: &mut Vec<AttrId>,
        typedef: bool,
    ) -> QualType {
        let mut ty = ty;
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if !matches!(node.kind, AttrKind::EXT_VECTOR_TYPE | AttrKind::VECTOR_SIZE) {
                i += 1;
                continue;
            }
            attrs.remove(i);
            let ext = node.kind == AttrKind::EXT_VECTOR_TYPE;
            // clang 22 applies `ext_vector_type` to any declared type
            // (spec/sema.md §1.1).
            if !typedef && ext && self.clang_compat < 22 {
                self.error(DiagId::sema_ext_vector_type_on_non_typedef, node.loc);
                continue;
            }
            if self.is_error_type(ty) {
                continue;
            }
            let kind = if ext { VectorKind::Ext } else { VectorKind::Generic };
            let Some(size) = ExprId::from_raw(node.a) else { continue };
            // clang `HandleVectorSizeAttr` / `HandleExtVectorTypeAttr`
            // leave the type unchanged after a diagnostic.
            let v = self.build_vector_type(ty, size, kind, node.loc);
            if v != QualType::ERROR {
                ty = v;
            }
        }
        ty
    }

    /// `base` with the vector attributes among `ds`'s decl-specifiers
    /// applied: they modify the decl-specifier type, not the declarator's
    /// (clang `processTypeAttrs(TAL_DeclSpec)`); the declarator's own
    /// trailing attributes apply to the whole declared type.
    pub(crate) fn apply_declspec_vector_attrs(
        &mut self,
        base: QualType,
        ds: &DeclSpec,
    ) -> QualType {
        let mut attrs = ds.attrs.clone();
        self.apply_vector_attrs(base, &mut attrs, ds.is_typedef())
    }

    /// `attrs` without the vector attributes (applied separately by
    /// [`Self::apply_declspec_vector_attrs`]).
    pub(crate) fn non_vector_attrs(&self, attrs: &[AttrId]) -> Vec<AttrId> {
        attrs
            .iter()
            .copied()
            .filter(|&a| {
                !matches!(
                    self.ast.attrs.get(a).kind,
                    AttrKind::EXT_VECTOR_TYPE | AttrKind::VECTOR_SIZE
                )
            })
            .collect()
    }

    /// The vector of `elem` sized by `size` (the element count for
    /// `VectorKind::Ext`, the byte size otherwise), clang's
    /// `BuildExtVectorType` / `BuildVectorType`; `ERROR` after a
    /// diagnostic.
    pub(crate) fn build_vector_type(
        &mut self,
        elem: QualType,
        size: ExprId,
        kind: VectorKind,
        loc: Loc,
    ) -> QualType {
        let ext = kind == VectorKind::Ext;
        let t = &self.ast.types;
        let elem_dep = t.is_dependent(elem);
        if !elem_dep {
            let ok = if ext {
                // An integer (`bool` included) or real floating type.
                t.is_integral(elem) || t.is_floating(elem)
            } else {
                !t.is_array(elem)
                    && t.builtin(elem).is_some()
                    && t.canonical(elem).ty() != QualType::BOOL.ty()
                    && (t.is_integral(elem) || t.is_floating(elem))
            };
            let already = matches!(t.canonical_view(elem), TypeView::Vector { .. });
            if !ok || already {
                let msg =
                    self.diag(DiagId::sema_vector_invalid_element_type, loc).arg(self.ty_arg(elem));
                self.emit(msg);
                return QualType::ERROR;
            }
        }
        if self.expr_is_value_dependent(size) || (!ext && elem_dep) {
            let p = self.dependent_profile(size);
            self.inst.vector_attr_locs.insert(size, loc);
            return self.ast.types.dependent_vector(elem, size, p, kind);
        }
        let name = if ext { "ext_vector_type" } else { "vector_size" };
        let Some(v) = self.vector_size_value(size, name, loc) else {
            return QualType::ERROR;
        };
        if v == 0 {
            self.error(DiagId::sema_vector_zero_size, loc);
            return QualType::ERROR;
        }
        let count = if ext {
            v
        } else {
            let esize = crate::target::size_align(&self.ast, elem, self.abi()).map_or(0, |s| s.0);
            if esize == 0 || v % u128::from(esize) != 0 {
                self.error(DiagId::sema_vector_size_not_multiple, loc);
                return QualType::ERROR;
            }
            v / u128::from(esize)
        };
        let Ok(count) = u32::try_from(count) else {
            self.error(DiagId::sema_vector_size_too_large, loc);
            return QualType::ERROR;
        };
        self.ast.types.vector(elem, count, kind)
    }

    /// The value of the size operand `e` of attribute `name`: an integral
    /// constant expression (clang `err_attribute_argument_type`).
    fn vector_size_value(&mut self, e: ExprId, name: &str, loc: Loc) -> Option<u128> {
        let ty = self.ast.exprs.get(e).ty();
        if self.is_error_type(ty) {
            return None;
        }
        if !self.is_integral_or_enum(ty) {
            let msg = self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(name);
            self.emit(msg);
            return None;
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_full_expr(c);
        match self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e))) {
            Ok(v) if v < 0 => {
                self.error(DiagId::sema_vector_size_too_large, loc);
                None
            }
            Ok(v) => u128::try_from(v).ok(),
            Err(_) => {
                let msg =
                    self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(name);
                self.emit(msg);
                None
            }
        }
    }
}

/// [`AttrKind::FORMAT`]'s `a`: a `printf` format, checked at calls.
pub(crate) const FORMAT_PRINTF: u32 = 0;
/// [`AttrKind::FORMAT`]'s `a`: a format clang supports that kiln does not
/// check (`scanf`, `strfmon`, ...).
pub(crate) const FORMAT_OTHER: u32 = 1;
/// [`AttrKind::FORMAT`]'s `a`: `strftime` (no data arguments).
pub(crate) const FORMAT_STRFTIME: u32 = 2;
