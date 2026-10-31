//! Performing user-defined conversions (spec/sema-overload.md §6.4):
//! the conversion sequence [`Sema::try_implicit_conversion`] chose is
//! applied to an expression, as clang's `PerformImplicitConversion` /
//! `InitializationSequence` do: a conversion function call wrapped in a
//! `UserDefinedConversion` cast, or a converting constructor call
//! wrapped in a `ConstructorConversion` cast, followed by the second
//! standard conversion (or reference binding).

use kiln_ast::expr::{CastKind, ValueCategory};
use kiln_ast::types::TypeView;
use kiln_ast::{ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use super::ics::{UdcVia, UserConv};
use super::{ArgInfo, Ics, IcsOptions, OverloadCandidateSet, SetKind};
use crate::Sema;
use crate::class::{ConstructSite, DeletedAs, DiagSites, InitStyle};
use crate::conv::{self, ConvCtx, ConvError};

/// How deeply user-defined conversions may nest while one is applied
/// (the second standard conversion never needs another; this only
/// guards against a disagreement between the two layers).
const MAX_UDC_DEPTH: u8 = 2;

/// The options of a user-defined conversion in context `ctx`: explicit
/// constructors and conversion functions are candidates in a
/// direct-initialization ([over.match.copy]/1.2, [over.match.conv]/1.2,
/// [over.match.ref]/1.1; a condition and a cast are ones).
pub(crate) fn udc_options(ctx: ConvCtx) -> IcsOptions {
    match ctx {
        ConvCtx::DirectInitialization | ConvCtx::Condition | ConvCtx::Cast => {
            IcsOptions::ALLOW_EXPLICIT
        }
        _ => IcsOptions::NONE,
    }
}

impl Sema<'_, '_> {
    /// Converts `e` to `to` (a type or a reference type) by the
    /// user-defined conversion its implicit conversion sequence names
    /// ([over.best.ics]); `at` is where clang reports the access and
    /// deletion errors (the variable of an initialization, else `e`).
    /// `None` when that sequence is not user-defined (nor ambiguous): the
    /// caller reports its own error.
    pub(crate) fn user_convert(
        &mut self,
        e: ExprId,
        to: QualType,
        ctx: ConvCtx,
        at: Loc,
    ) -> Option<Result<ExprId, ConvError>> {
        if self.ovl.udc_depth >= MAX_UDC_DEPTH {
            return None;
        }
        let a = self.arg_info(e);
        let opts = udc_options(ctx);
        match self.try_implicit_conversion(&a, to, opts) {
            Ics::UserDefined(u) => {
                self.ovl.udc_depth += 1;
                let r = self.apply_udc(e, &u, to, ctx, at);
                self.ovl.udc_depth -= 1;
                Some(r)
            }
            Ics::Ambiguous { .. } => Some(Err(self.ambiguous_conversion(&a, to, opts))),
            _ => None,
        }
    }

    /// Applies the user-defined conversion `u` of `e` to `to`.
    fn apply_udc(
        &mut self,
        e: ExprId,
        u: &UserConv,
        to: QualType,
        ctx: ConvCtx,
        at: Loc,
    ) -> Result<ExprId, ConvError> {
        let (Some(f), Some(found)) = (u.func, u.found) else {
            return Err(unsupported_udc());
        };
        let x = match u.via {
            UdcVia::Aggregate => return Err(unsupported_udc()),
            UdcVia::ConvFn => {
                let target = self.non_reference(to);
                self.conversion_function_call(e, f, found, target, at)?
            }
            UdcVia::Ctor => {
                let referee = self.non_reference(to);
                if self.ast.types.is_reference(to) {
                    // [dcl.init.ref]/5.4.1: the temporary is constructed
                    // as cv1 T1 directly (clang's `CXXConstructExpr
                    // 'const A'`).
                    let site = ConstructSite::at(self.begin_loc(e));
                    let site = ConstructSite {
                        sites: DiagSites {
                            access: at,
                            deleted: at,
                            deleted_as: DeletedAs::Conversion(None),
                        },
                        ..site
                    };
                    match self.construct_class(referee, &[e], InitStyle::Copy, site) {
                        Some(c) => c,
                        None => return Err(no_conversion(self, e, to)),
                    }
                } else {
                    match self.construct_converted(
                        e,
                        conv::strip_cv(self, referee),
                        ctx,
                        at,
                        DeletedAs::Call,
                    ) {
                        Some(c) => c,
                        None => return Err(no_conversion(self, e, to)),
                    }
                }
            }
        };
        if self.is_err_expr(x) {
            return Ok(x);
        }
        if self.ast.types.is_reference(to)
            && u.via == UdcVia::Ctor
            && self.ast.exprs.get(x).value_category() == ValueCategory::PRValue
        {
            let rvalue = matches!(self.ast.types.canonical_view(to), TypeView::RValueRef(_));
            let ty = self.ety(x);
            return Ok(self.materialize_temporary(x, ty, !rvalue));
        }
        conv::implicit_convert(self, x, to, ctx)
    }

    /// `e.operator T()` for the conversion function `f` (found as
    /// `found`), wrapped in a `UserDefinedConversion` cast of the call's
    /// type and category: access and deletion are checked at `at`.
    pub(crate) fn call_conversion_function(
        &mut self,
        e: ExprId,
        f: kiln_ast::DeclId,
        found: kiln_ast::DeclId,
        at: Loc,
    ) -> ExprId {
        let target = self.conversion_result_type(f);
        let target = self.non_reference(target);
        match self.conversion_function_call(e, f, found, target, at) {
            Ok(x) => x,
            Err(err) => {
                let msg = err.diag_at(self, at);
                self.emit(msg);
                let loc = self.eloc(e);
                self.recover_err(&[e], loc)
            }
        }
    }

    /// [`Sema::call_conversion_function`], a deleted function's error
    /// returned (for the caller's notes) rather than emitted.
    fn conversion_function_call(
        &mut self,
        e: ExprId,
        f: kiln_ast::DeclId,
        found: kiln_ast::DeclId,
        target: QualType,
        at: Loc,
    ) -> Result<ExprId, ConvError> {
        let loc = self.eloc(e);
        let src = self.ety(e);
        let class = self.ast.types.as_record(src).and_then(|r| self.ast.definition(r)).unwrap_or(f);
        self.check_member_access(found, class, Some(class), at);
        if self.ovl_deleted(f) {
            let mut err = ConvError::new(
                DiagId::sema_conv_deleted_ctor,
                vec![self.ty_arg(src), self.ty_arg(target)],
            );
            err.notes = self.deleted_function_notes(f, 0);
            return Err(err);
        }
        // clang's `PerformImplicitObjectArgumentInitialization`
        // materializes a prvalue object as an lvalue unless the function
        // is `&&`-qualified.
        let e = if self.ast.exprs.get(e).value_category() == ValueCategory::PRValue {
            let (_, rq) = self.method_object_quals(f);
            self.materialize_temporary(e, src, rq != kiln_ast::types::RefQual::RValue)
        } else {
            e
        };
        let shadow = (found != f).then_some(found);
        let mem = self.member_of_object(e, src, class, false, f, shadow, None, loc);
        if self.is_err_expr(mem) {
            return Ok(mem);
        }
        let Some(call) = self.try_member_call(mem, &[], &mut [], loc, loc) else {
            return Ok(self.recover_err(&[mem], loc));
        };
        if self.is_err_expr(call) {
            return Ok(call);
        }
        let n = *self.ast.exprs.get(call);
        Ok(conv::raw_cast(
            self,
            CastKind::UserDefinedConversion,
            call,
            n.ty(),
            n.value_category(),
            None,
        ))
    }

    /// The declared result type of conversion function `f` (`T` of
    /// `operator T`).
    pub(crate) fn conversion_result_type(&self, f: kiln_ast::DeclId) -> QualType {
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret)
    }

    /// clang's ambiguity error for converting `a` to `to`, with a note
    /// per ambiguous candidate.
    pub(crate) fn ambiguous_conversion(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        opts: IcsOptions,
    ) -> ConvError {
        let mut err = if self.ast.types.is_reference(to) {
            ConvError::new(
                DiagId::sema_ref_init_ambiguous,
                vec![self.ty_arg(to), self.ty_arg(a.ty)],
            )
        } else {
            ConvError::new(
                DiagId::sema_ovl_ambiguous_conversion,
                vec![self.ty_arg(a.ty), self.ty_arg(to)],
            )
        };
        err.notes = self.udc_notes_of(a, to, opts, true);
        err
    }

    /// The candidate notes of a failed user-defined conversion of `e` to
    /// `to` in context `ctx` (clang's "no viable conversion" notes):
    /// empty when neither type is a class.
    pub(crate) fn udc_failure_notes(&mut self, e: ExprId, to: QualType, ctx: ConvCtx) -> Vec<Diag> {
        let a = self.arg_info(e);
        self.udc_notes_of(&a, to, udc_options(ctx), false)
    }

    /// The notes of a failed cast's user-defined conversion of `a` to
    /// `to` (explicit functions are candidates): with `viable_only` the
    /// viable candidates (ambiguous or deleted best), else every one.
    pub(crate) fn udc_cast_notes(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        viable_only: bool,
    ) -> Vec<Diag> {
        let Some(mut set) = self.udc_set(a, to, IcsOptions::ALLOW_EXPLICIT) else {
            return Vec::new();
        };
        let _ = self.best_viable(&mut set);
        let notes = if viable_only {
            let mut order: Vec<u32> = (0..u32::try_from(set.cands.len()).unwrap_or(u32::MAX))
                .filter(|&c| set.cand(c).viable)
                .collect();
            order.sort_by_key(|&c| self.ast.decls.get(set.cand(c).func()).loc());
            order.into_iter().map(|c| self.ovl_note(&set, c)).collect()
        } else {
            self.ovl_notes(&set, false)
        };
        self.end_set(set);
        notes
    }

    pub(crate) fn udc_notes_of(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        opts: IcsOptions,
        ambiguous: bool,
    ) -> Vec<Diag> {
        let Some(mut set) = self.udc_set(a, to, opts) else { return Vec::new() };
        let _ = self.best_viable(&mut set);
        let notes = self.ovl_notes(&set, ambiguous);
        self.end_set(set);
        notes
    }

    /// The candidate set a user-defined conversion of `a` to `to`
    /// resolves (the one [`Sema::try_implicit_conversion`] decided by),
    /// for its notes; `None` when there is none.
    fn udc_set(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        opts: IcsOptions,
    ) -> Option<OverloadCandidateSet> {
        let class_src = self.ast.types.as_record(a.ty).is_some()
            && self.require_complete_silent(a.ty, Loc::INVALID);
        let (lref, referee) = match self.ast.types.canonical_view(to) {
            TypeView::LValueRef(p) => (Some(true), p),
            TypeView::RValueRef(p) => (Some(false), p),
            _ => (None, to),
        };
        if let Some(lref) = lref {
            let q1 = self.ast.types.effective_quals(referee);
            let plain = lref && (!q1.is_const() || q1.is_volatile());
            if class_src {
                for lvalue in [true, false] {
                    if (lvalue && !lref) || (!lvalue && plain) {
                        continue;
                    }
                    let mut set = self.begin_set(SetKind::ConvToRef { lvalue }, Loc::INVALID);
                    set.target = to;
                    set.args.push(*a);
                    for (found, f) in self.conversion_functions(a.ty) {
                        self.add_conversion_candidate(&mut set, f, found, to, opts);
                    }
                    if set.viable().next().is_some() {
                        return Some(set);
                    }
                    self.end_set(set);
                }
            }
            if plain {
                return None;
            }
        }
        let target = if lref.is_some() { conv::strip_cv(self, referee) } else { to };
        let class_dst = self.ast.types.as_record(target).is_some();
        if !class_src && !class_dst {
            return None;
        }
        let kind = if class_dst { SetKind::CopyInit } else { SetKind::ConvToNonClass };
        let mut set = self.begin_set(kind, Loc::INVALID);
        set.target = target;
        self.build_conversion_set(&mut set, a, target, opts);
        Some(set)
    }
}

fn unsupported_udc() -> ConvError {
    ConvError::new(DiagId::sema_conv_unsupported, vec!["this user-defined conversion".into()])
}

fn no_conversion(s: &mut Sema<'_, '_>, e: ExprId, to: QualType) -> ConvError {
    let (fa, ta) = (s.ty_arg(s.ety(e)), s.ty_arg(to));
    ConvError::new(DiagId::sema_no_viable_conversion, vec![fa, ta])
}
