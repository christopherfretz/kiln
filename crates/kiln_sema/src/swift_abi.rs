//! The Swift parameter ABI attributes `swift_context`,
//! `swift_async_context`, `swift_error_result` and
//! `swift_indirect_result` (clang `ParameterABIAttr`; spec/sema.md §4.5,
//! WP-1745). Each is checked against its parameter's type where the
//! parameter is declared (clang `SemaSwift::AddParameterABIAttr`), then
//! recorded in the function type ([`kiln_ast::types::FuncInfo::param_abis`],
//! clang's `ExtParameterInfo`), whose calling convention and parameter
//! order are checked once the convention is known (clang
//! `checkExtParameterInfos`): a convention written at a pointer,
//! reference or grouping chunk comes too late for that (WP-1827). A
//! substituted function type's convention and order are checked again
//! for its expanded parameters (clang `BuildFunctionType`, WP-1810).

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::types::{BuiltinType, CallConv, FuncInfo, ParamAbi, TypeView};
use kiln_ast::{AttrId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::ParamInfo;

/// A function type whose parameter ABIs wait for its calling convention
/// ([`Sema::apply_cc_attrs`] applies the declaration's attributes after
/// the declarator's type is built).
#[derive(Clone, Debug)]
pub(crate) struct PendingParamAbis {
    /// The function type as built (its parameter types and ABI list).
    pub(crate) info: FuncInfo,
    pub(crate) params: Vec<QualType>,
    /// Each parameter's location.
    pub(crate) locs: Vec<Loc>,
}

/// The ABI of a [`AttrKind::SWIFT_PARAM_ABI`] node.
#[allow(clippy::cast_possible_truncation, reason = "a ParamAbi")]
fn abi_of(node: &AttrNode) -> ParamAbi {
    ParamAbi::from_u8(node.a as u8)
}

impl Sema<'_, '_> {
    /// clang `SemaSwift::AddParameterABIAttr` for the parameter ABI
    /// attributes among a parameter's `attrs` (its adjusted type `ty`): a
    /// second, different one is "X and 'Y' attributes are not
    /// compatible" (with a note at the first) and dropped; one whose
    /// type cannot carry it is "'X' parameter must have pointer type"
    /// and kept; `false` after one.
    pub(crate) fn check_param_abi_attrs(&mut self, ty: QualType, attrs: &mut Vec<AttrId>) -> bool {
        let mut first: Option<AttrNode> = None;
        let mut valid = true;
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if node.kind != AttrKind::SWIFT_PARAM_ABI {
                i += 1;
                continue;
            }
            let abi = abi_of(&node);
            if let Some(prev) = first
                && abi_of(&prev) != abi
            {
                let shown = format!("'{}'", self.attr_shown(&prev, abi_of(&prev).spelling()));
                let prev_note = self.diag(DiagId::note_conflicting_attribute, prev.loc);
                let msg = self
                    .diag(DiagId::sema_cconv_incompatible, node.loc)
                    .arg(abi.spelling())
                    .arg(shown)
                    .with_note(prev_note);
                self.emit(msg);
                attrs.remove(i);
                continue;
            }
            first.get_or_insert(node);
            valid &= self.param_abi_type_ok(abi, ty, node.loc);
            i += 1;
        }
        valid
    }

    /// The parameter ABI attributes of parameter declaration `d`, which
    /// its instantiations carry (clang `SubstParmVarDecl` →
    /// `InstantiateAttrs`).
    pub(crate) fn param_abi_attrs(&self, d: kiln_ast::DeclId) -> Vec<AttrId> {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(d))
            .filter(|&a| self.ast.attrs.get(a).kind == AttrKind::SWIFT_PARAM_ABI)
            .collect()
    }

    /// `info` with the parameter ABIs of `params` (an instantiated
    /// function's, a pack expanded into one parameter each), when the
    /// pattern's has any.
    pub(crate) fn info_with_param_abis(
        &mut self,
        info: FuncInfo,
        params: &[ParamInfo],
    ) -> FuncInfo {
        if info.param_abis() == 0 {
            return info;
        }
        let abis = self.declarator_param_abis(params);
        info.with_param_abis(self.ast.types.intern_param_abis(&abis))
    }

    /// Whether parameter type `ty` can carry `abi`; if not, "'X'
    /// parameter must have pointer type" at `loc`.
    pub(crate) fn param_abi_type_ok(&mut self, abi: ParamAbi, ty: QualType, loc: Loc) -> bool {
        let ok = match abi {
            ParamAbi::Ordinary => true,
            ParamAbi::SwiftContext | ParamAbi::SwiftAsyncContext => self.swift_context_type(ty),
            ParamAbi::SwiftErrorResult => self.swift_error_result_type(ty),
            ParamAbi::SwiftIndirectResult => self.swift_indirect_result_type(ty),
        };
        if !ok {
            let to_ptr = if abi == ParamAbi::SwiftErrorResult { "1" } else { "0" };
            let msg = self
                .diag(DiagId::sema_swift_abi_parameter_wrong_type, loc)
                .arg(abi.spelling())
                .arg(to_ptr)
                .arg(self.ty_arg(ty));
            self.emit(msg);
        }
        ok
    }

    /// clang `isValidSwiftContextType`: a type with a pointer
    /// representation (every kiln pointer is in the default address
    /// space), or a dependent one.
    fn swift_context_type(&self, ty: QualType) -> bool {
        let t = &self.ast.types;
        match t.canonical_view(ty) {
            TypeView::Pointer(_)
            | TypeView::LValueRef(_)
            | TypeView::RValueRef(_)
            | TypeView::Builtin(BuiltinType::NullPtr) => true,
            _ => t.is_dependent(ty),
        }
    }

    /// The pointee of a pointer or reference type `ty`.
    fn swift_pointee(&self, ty: QualType) -> Option<QualType> {
        match self.ast.types.canonical_view(ty) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => Some(p),
            _ => None,
        }
    }

    /// clang `isValidSwiftIndirectResultType`: a pointer or reference.
    fn swift_indirect_result_type(&self, ty: QualType) -> bool {
        self.swift_pointee(ty).is_some() || self.ast.types.is_dependent(ty)
    }

    /// clang `isValidSwiftErrorResultType`: a pointer or reference to an
    /// unqualified [`Self::swift_context_type`].
    fn swift_error_result_type(&self, ty: QualType) -> bool {
        match self.swift_pointee(ty) {
            Some(p) => self.ast.types.effective_quals(p).is_empty() && self.swift_context_type(p),
            None => self.ast.types.is_dependent(ty),
        }
    }

    /// The parameter ABIs of a function declarator's `params` (clang
    /// `GetFullTypeForDeclarator`: each parameter's first
    /// `ParameterABIAttr`); empty when every one is ordinary.
    pub(crate) fn declarator_param_abis(&self, params: &[ParamInfo]) -> Vec<ParamAbi> {
        let abis: Vec<ParamAbi> = params
            .iter()
            .map(|p| {
                p.attrs
                    .iter()
                    .map(|&a| self.ast.attrs.get(a))
                    .find(|n| n.kind == AttrKind::SWIFT_PARAM_ABI)
                    .map_or(ParamAbi::Ordinary, abi_of)
            })
            .collect();
        if abis.iter().all(|&a| a == ParamAbi::Ordinary) { Vec::new() } else { abis }
    }

    /// clang `checkExtParameterInfos` of a function type with calling
    /// convention `cc` and parameter ABIs `abis` (parameter `i` at
    /// `locs[i]`): `swift_indirect_result` parameters come first and
    /// `swift_error_result` follows `swift_context`; all but
    /// `swift_async_context` need `swiftcall` (or `swiftasynccall`,
    /// except for `swift_error_result`), reported once. False after an
    /// error.
    pub(crate) fn check_ext_param_infos(
        &mut self,
        cc: CallConv,
        abis: &[ParamAbi],
        locs: &[Loc],
    ) -> bool {
        let mut reported = false;
        let mut ok = true;
        let at = |i: usize| locs.get(i).copied().unwrap_or(Loc::INVALID);
        let prev = |i: usize| if i == 0 { ParamAbi::Ordinary } else { abis[i - 1] };
        for (i, &abi) in abis.iter().enumerate() {
            let only_swift = match abi {
                ParamAbi::Ordinary | ParamAbi::SwiftAsyncContext => continue,
                ParamAbi::SwiftErrorResult => true,
                ParamAbi::SwiftIndirectResult | ParamAbi::SwiftContext => false,
            };
            let cc_ok = cc == CallConv::Swift || (!only_swift && cc == CallConv::SwiftAsync);
            if !cc_ok && !reported {
                let msg = self
                    .diag(DiagId::sema_swift_param_attr_not_swiftcall, at(i))
                    .arg(abi.spelling())
                    .arg(if only_swift { "1" } else { "0" });
                self.emit(msg);
                reported = true;
            }
            ok &= cc_ok;
            match abi {
                ParamAbi::SwiftIndirectResult
                    if i != 0 && prev(i) != ParamAbi::SwiftIndirectResult =>
                {
                    self.error(DiagId::sema_swift_indirect_result_not_first, at(i));
                    ok = false;
                }
                ParamAbi::SwiftErrorResult if prev(i) != ParamAbi::SwiftContext => {
                    self.error(DiagId::sema_swift_error_result_not_after_swift_context, at(i));
                    ok = false;
                }
                _ => {}
            }
        }
        ok
    }

    /// The function chunk of a declarator with parameters `params` built
    /// as `fty` (with `info`'s parameter ABIs): checked now with the
    /// convention it has, or, when the declaration's attributes may still
    /// give it one (`deferred`: the declarator's own function type, or
    /// the one its single pointer, reference or member pointer chunk
    /// points to), when [`Sema::apply_cc_attrs`] has.
    pub(crate) fn param_abis_built(&mut self, fty: QualType, params: &[ParamInfo], deferred: bool) {
        let Some(p) = self.ast.types.proto(fty).copied() else { return };
        let info = p.info();
        // An instantiation's declarator is its pattern's, checked where
        // written; its calling convention is set after it is built
        // (`Subst::function_decl`'s `keep`).
        if info.param_abis() == 0 || !self.inst.stack.is_empty() {
            return;
        }
        let locs: Vec<Loc> = params
            .iter()
            .map(|p| {
                p.decl.map_or_else(
                    || p.name.map_or(p.loc, |n| n.1),
                    |pd| self.ast.decls.get(pd).loc(),
                )
            })
            .collect();
        let pending = PendingParamAbis { info, params: self.ast.types.params(&p).to_vec(), locs };
        if deferred {
            self.pending_param_abis.push(pending);
        } else {
            self.check_pending_param_abis(&pending, info.cc());
        }
    }

    fn check_pending_param_abis(&mut self, p: &PendingParamAbis, cc: CallConv) {
        let abis = self.ast.types.param_abi_list(p.info);
        let _ = self.check_ext_param_infos(cc, &abis, &p.locs);
    }

    /// Checks every pending function type with the convention it was
    /// built with: no attribute gives it another one now.
    pub(crate) fn flush_pending_param_abis(&mut self) {
        for p in std::mem::take(&mut self.pending_param_abis) {
            self.check_pending_param_abis(&p, p.info.cc());
        }
    }

    /// After [`Sema::apply_cc_attrs`] gave the function type of `ty` (or
    /// the one it points, refers or member-points to) its convention: the
    /// pending function type it was built as is checked once it has a
    /// convention, or when no attribute can give it one any more
    /// (`last`: the declaration's own attributes were applied), and then
    /// so is every other pending one, with its own.
    pub(crate) fn param_abis_after_cc(&mut self, ty: QualType, last: bool) {
        self.param_abis_reached(ty, last, false);
    }

    /// The pending function type `ty` reaches checked now, with the
    /// convention it has: the function type attributes written at a
    /// pointer, reference or grouping chunk apply after it was built
    /// (clang `processTypeAttrs` on that chunk; WP-1827).
    pub(crate) fn param_abis_before_chunk_attrs(&mut self, ty: QualType) {
        self.param_abis_reached(ty, false, true);
    }

    fn param_abis_reached(&mut self, ty: QualType, last: bool, now: bool) {
        if self.pending_param_abis.is_empty() {
            return;
        }
        let reached = self.fn_type_through(ty);
        let t = &self.ast.types;
        let found = reached.and_then(|f| t.proto(f)).and_then(|p| {
            let params = t.params(p);
            let id = p.info().param_abis();
            self.pending_param_abis
                .iter()
                .rposition(|x| x.info.param_abis() == id && x.params.as_slice() == params)
                .map(|i| (i, p.info().cc()))
        });
        if let Some((i, cc)) = found
            && (last || now || cc != CallConv::Default)
        {
            let p = self.pending_param_abis.remove(i);
            self.check_pending_param_abis(&p, cc);
        }
        if last {
            self.flush_pending_param_abis();
        }
    }
}
