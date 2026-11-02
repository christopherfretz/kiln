//! The contextual implicit conversion of a `switch` condition of class
//! type ([stmt.switch]/2, [conv.general]/5; clang
//! `PerformContextualImplicitConversion` with the C++14 rules).
//!
//! The class's conversion functions whose (non-reference) result type is
//! integral or enumeration are collected. The non-explicit, non-template
//! ones must agree on one target type `T` (otherwise "multiple
//! conversions", one note per conversion); overload resolution among the
//! matching non-explicit conversion functions (templates deduced against
//! `T`) then picks the one called. With none usable and exactly one
//! explicit matching conversion function, clang's "requires explicit
//! conversion" error is reported and that function is called anyway.

use kiln_ast::decl::DeclKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::overload::{IcsOptions, OvlOutcome, SetKind};

impl Sema<'_, '_> {
    /// Converts the class-type `switch` condition `cond` (`switch` keyword
    /// at `loc`) to an integral or enumeration type. `None` when no
    /// conversion function applies (the caller reports clang's "statement
    /// requires expression of integer type"); a reported error yields an
    /// error expression.
    pub(crate) fn switch_class_condition(&mut self, cond: ExprId, loc: Loc) -> Option<ExprId> {
        let ty = self.ety(cond);
        if !self.require_complete(ty, loc) {
            let d = self.diag(DiagId::sema_switch_incomplete_class_type, loc).arg(self.ty_arg(ty));
            let d = self.incomplete_note(d, ty);
            self.emit(d);
            return Some(self.recover_err(&[cond], loc));
        }
        let mut viable: Vec<(DeclId, DeclId)> = Vec::new();
        let mut explicit: Vec<(DeclId, DeclId)> = Vec::new();
        let mut target: Option<QualType> = None;
        let mut unique = true;
        for (found, m) in self.conversion_functions_all(ty) {
            let tmpl = self.kind_of(m) == DeclKind::FunctionTemplate;
            let f = if tmpl {
                match self.templated_function(m) {
                    Some(f) => f,
                    None => continue,
                }
            } else {
                m
            };
            let to = self.switch_conversion_type(f);
            if !tmpl && !self.integral_or_enum(to) {
                continue;
            }
            if self.fn_spec(f).is_explicit() {
                explicit.push((found, m));
                continue;
            }
            if !tmpl {
                let u = self.ast.types.canonical(conv::strip_cv(self, to));
                match target {
                    None => target = Some(u),
                    Some(t) if t != u => unique = false,
                    Some(_) => {}
                }
            }
            viable.push((found, m));
        }
        if let Some(t) = target {
            if !unique {
                self.report_multiple_conversions(cond, &viable, loc);
                return Some(self.recover_err(&[cond], loc));
            }
            if let Some(e) = self.switch_resolve_conversion(cond, &viable, t, loc) {
                return Some(e);
            }
        }
        // clang `diagnoseNoViableConversion`: a single explicit candidate
        // is reported and called (recovery).
        if let [(found, m)] = explicit[..] {
            let f = self.templated_function(m).unwrap_or(m);
            if self.kind_of(m) != DeclKind::FunctionTemplate {
                let to = self.switch_conversion_type(f);
                let d = self
                    .diag(DiagId::sema_switch_explicit_conversion, loc)
                    .arg(self.ty_arg(ty))
                    .arg(self.ty_arg(to))
                    .with_note(self.note_switch_conversion(f));
                self.emit(d);
                let call = self.call_conversion_function(cond, f, found, loc);
                return Some(self.switch_finish_conversion(call, to));
            }
        }
        None
    }

    /// Overload resolution among the non-explicit matching conversion
    /// functions `viable` for the target `t` ([over.match.conv]);
    /// `None` when none is viable or the best is deleted.
    fn switch_resolve_conversion(
        &mut self,
        cond: ExprId,
        viable: &[(DeclId, DeclId)],
        t: QualType,
        loc: Loc,
    ) -> Option<ExprId> {
        let a = self.arg_info(cond);
        let mut set = self.begin_set(SetKind::ConvToNonClass, loc);
        set.target = t;
        set.args.push(a);
        for &(found, m) in viable {
            self.add_conversion_candidate(&mut set, m, found, t, IcsOptions::NONE);
        }
        self.order_conversion_templates(&mut set);
        self.order_template_candidates(&mut set);
        let best = match self.best_viable(&mut set) {
            OvlOutcome::Success(c) => Some((set.cand(c).func(), set.cand(c).found)),
            OvlOutcome::Ambiguous => {
                // Distinct functions yielding the same `T` (clang
                // `diagnoseAmbiguousConversion`).
                self.report_multiple_conversions(cond, viable, loc);
                self.end_set(set);
                return Some(self.recover_err(&[cond], loc));
            }
            OvlOutcome::NoViable | OvlOutcome::Deleted(_) => None,
        };
        self.end_set(set);
        let (f, found) = best?;
        let call = self.call_conversion_function(cond, f, found, loc);
        Some(self.switch_finish_conversion(call, t))
    }

    /// The result of the conversion function call `call` as a prvalue of
    /// type `to` (an lvalue-to-rvalue conversion for a reference result).
    fn switch_finish_conversion(&mut self, call: ExprId, to: QualType) -> ExprId {
        if self.is_err_expr(call) {
            return call;
        }
        let to = conv::strip_cv(self, to);
        if let Ok(e) = conv::implicit_convert(self, call, to, ConvCtx::Initialization) {
            e
        } else {
            let loc = self.eloc(call);
            self.recover_err(&[call], loc)
        }
    }

    /// The non-reference result type of conversion function `f`.
    fn switch_conversion_type(&self, f: DeclId) -> QualType {
        let r = self.conversion_result_type(f);
        self.non_reference(r)
    }

    /// Whether `t` is an integral or enumeration type (clang
    /// `isIntegralOrEnumerationType`, scoped enumerations included).
    fn integral_or_enum(&self, t: QualType) -> bool {
        let t = self.ast.types.canonical(t);
        self.ast.types.is_integral(t) || self.ast.types.as_enum(t).is_some()
    }

    /// clang's "multiple conversions" error at `loc`, one note per
    /// non-template conversion function of `viable`.
    fn report_multiple_conversions(&mut self, cond: ExprId, viable: &[(DeclId, DeclId)], loc: Loc) {
        let ty = self.ety(cond);
        let mut d = self.diag(DiagId::sema_switch_multiple_conversions, loc).arg(self.ty_arg(ty));
        for &(_, m) in viable {
            if self.kind_of(m) != DeclKind::FunctionTemplate {
                d = d.with_note(self.note_switch_conversion(m));
            }
        }
        self.emit(d);
    }

    /// clang's "conversion to integral type 'T'" note at conversion
    /// function `f`.
    fn note_switch_conversion(&self, f: DeclId) -> kiln_diag::Diag {
        let to = self.switch_conversion_type(f);
        let is_enum = self.ast.types.as_enum(self.ast.types.canonical(to)).is_some();
        let at = self.ast.decls.get(f).loc();
        self.diag(DiagId::note_switch_conversion, at)
            .arg(if is_enum { "1" } else { "0" })
            .arg(self.ty_arg(to))
    }
}
