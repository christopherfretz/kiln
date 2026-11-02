//! The contextual implicit conversion of an operand of class type to a
//! type of a required kind ([conv.general]/5; clang
//! `PerformContextualImplicitConversion` with the C++14 rules): a
//! `switch` condition ([stmt.switch]/2, integral or enumeration), a
//! pre-C++14 new-expression array bound ([expr.new]/6 in C++11, integral
//! or unscoped enumeration) and a `delete` operand ([expr.delete]/2,
//! pointer to object type). One helper serves all three, so their
//! diagnostics differ only in wording ([`ContextualConv`]).
//!
//! The class's conversion functions whose (non-reference) result type
//! matches are collected. The non-explicit, non-template ones must agree
//! on one target type `T` (otherwise clang's "multiple"/"ambiguous
//! conversions" error, one note per conversion function); overload
//! resolution among the matching non-explicit conversion functions
//! (templates deduced against `T`) then picks the one called. With none
//! usable and exactly one explicit matching conversion function, clang's
//! "requires explicit conversion" error is reported and that function is
//! called anyway.
//!
//! A conversion function template among the conversions of an ambiguity
//! is noted "candidate function [with T = int]" (its specialization
//! deduced against `T`), or "candidate function template" when it was not
//! deduced. clang 19-22 crash there (`diagnoseAmbiguousConversion` casts
//! the template to a conversion function), so this wording is kiln's.

use kiln_ast::decl::DeclKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::overload::{IcsOptions, OvlOutcome, SetKind};

/// The operand a contextual implicit conversion applies to; it selects
/// the target types and clang's diagnostic wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextualConv {
    /// A `switch` condition: integral or enumeration type.
    Switch,
    /// A pre-C++14 new-expression array bound: integral or unscoped
    /// enumeration type.
    ArraySize,
    /// A `delete` operand: pointer to object (or incomplete) type.
    Delete,
}

impl ContextualConv {
    fn incomplete(self) -> DiagId {
        match self {
            ContextualConv::Switch => DiagId::sema_switch_incomplete_class_type,
            ContextualConv::ArraySize => DiagId::sema_array_size_incomplete_type,
            ContextualConv::Delete => DiagId::sema_delete_incomplete_class,
        }
    }

    fn explicit(self) -> DiagId {
        match self {
            ContextualConv::Switch => DiagId::sema_switch_explicit_conversion,
            ContextualConv::ArraySize => DiagId::sema_array_size_explicit_conversion,
            ContextualConv::Delete => DiagId::sema_delete_explicit_conversion,
        }
    }

    fn ambiguous(self) -> DiagId {
        match self {
            ContextualConv::Switch => DiagId::sema_switch_multiple_conversions,
            ContextualConv::ArraySize => DiagId::sema_array_size_ambiguous_conversion,
            ContextualConv::Delete => DiagId::sema_delete_ambiguous_conversion,
        }
    }
}

impl Sema<'_, '_> {
    /// Contextually implicitly converts the class-type operand `cond` for
    /// `kind` (diagnostics at `loc`: the `switch`, `new` or `delete`
    /// keyword). `None` when no conversion function applies (the caller
    /// reports its "no match" error); a reported error yields an error
    /// expression.
    pub(crate) fn contextual_class_conversion(
        &mut self,
        kind: ContextualConv,
        cond: ExprId,
        loc: Loc,
    ) -> Option<ExprId> {
        let ty = self.ety(cond);
        if !self.require_complete(ty, loc) {
            let d = self.diag(kind.incomplete(), loc).arg(self.ty_arg(ty));
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
            let to = self.contextual_conversion_type(f);
            if !tmpl && !self.contextual_target_matches(kind, to) {
                continue;
            }
            if self.fn_spec(f).is_explicit() {
                // clang counts only the non-template explicit ones.
                if !tmpl {
                    explicit.push((found, m));
                }
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
                let notes = self.contextual_conversion_notes(kind, &viable, None);
                return Some(self.report_ambiguous_conversion(kind, cond, notes, loc));
            }
            if let Some(e) = self.contextual_resolve_conversion(kind, cond, &viable, t, loc) {
                return Some(e);
            }
        }
        // clang `diagnoseNoViableConversion`: a single explicit candidate
        // is reported and called (recovery).
        if let [(found, f)] = explicit[..] {
            let to = self.contextual_conversion_type(f);
            let d = self
                .diag(kind.explicit(), loc)
                .arg(self.ty_arg(ty))
                .arg(self.ty_arg(to))
                .with_note(self.note_contextual_conversion(kind, f));
            self.emit(d);
            let call = self.call_conversion_function(cond, f, found, loc);
            return Some(self.contextual_finish_conversion(call, to));
        }
        None
    }

    /// Overload resolution among the non-explicit matching conversion
    /// functions `viable` for the target `t` ([over.match.conv]);
    /// `None` when none is viable or the best is deleted.
    fn contextual_resolve_conversion(
        &mut self,
        kind: ContextualConv,
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
                let notes = self.contextual_conversion_notes(kind, viable, Some(&set));
                self.end_set(set);
                return Some(self.report_ambiguous_conversion(kind, cond, notes, loc));
            }
            OvlOutcome::NoViable | OvlOutcome::Deleted(_) => None,
        };
        self.end_set(set);
        let (f, found) = best?;
        let call = self.call_conversion_function(cond, f, found, loc);
        Some(self.contextual_finish_conversion(call, t))
    }

    /// The result of the conversion function call `call` as a prvalue of
    /// type `to` (an lvalue-to-rvalue conversion for a reference result).
    fn contextual_finish_conversion(&mut self, call: ExprId, to: QualType) -> ExprId {
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
    fn contextual_conversion_type(&self, f: DeclId) -> QualType {
        let r = self.conversion_result_type(f);
        self.non_reference(r)
    }

    /// Whether `to` is a target `kind` accepts (clang's converter
    /// `match`).
    pub(crate) fn contextual_target_matches(&self, kind: ContextualConv, to: QualType) -> bool {
        let types = &self.ast.types;
        let t = types.canonical(to);
        match kind {
            ContextualConv::Switch => types.is_integral(t) || types.as_enum(t).is_some(),
            ContextualConv::ArraySize => {
                types.is_integral(t) || types.as_enum(t).is_some_and(|en| !self.is_scoped_enum(en))
            }
            // A pointer to an object or incomplete type (`void *`
            // included), not to a function.
            ContextualConv::Delete => {
                types.is_pointer(t) && types.pointee(t).is_some_and(|p| !types.is_function(p))
            }
        }
    }

    /// The notes of an ambiguity among `viable`: clang's per-context note
    /// for a non-template conversion function; for a template, the
    /// candidate note of its specialization in `set` ("candidate
    /// function [with T = int]"), or "candidate function template" when
    /// it was not deduced (see the module docs).
    fn contextual_conversion_notes(
        &mut self,
        kind: ContextualConv,
        viable: &[(DeclId, DeclId)],
        set: Option<&crate::overload::OverloadCandidateSet>,
    ) -> Vec<Diag> {
        let mut notes = Vec::with_capacity(viable.len());
        for &(_, m) in viable {
            if self.kind_of(m) != DeclKind::FunctionTemplate {
                notes.push(self.note_contextual_conversion(kind, m));
                continue;
            }
            let cand = set.and_then(|set| {
                (0..u32::try_from(set.cands.len()).unwrap_or(u32::MAX))
                    .find(|&c| set.cand(c).template == Some(m) && set.cand(c).viable)
                    .map(|c| (set, c))
            });
            if let Some((set, c)) = cand {
                notes.push(self.ovl_note(set, c));
            } else {
                let at = self.ast.decls.get(m).loc();
                notes.push(self.diag(DiagId::note_ovl_candidate, at).arg("function template"));
            }
        }
        notes
    }

    /// clang's ambiguous-conversion error for `kind` at `loc` with
    /// `notes`; the error expression.
    fn report_ambiguous_conversion(
        &mut self,
        kind: ContextualConv,
        cond: ExprId,
        notes: Vec<Diag>,
        loc: Loc,
    ) -> ExprId {
        let ty = self.ety(cond);
        let mut d = self.diag(kind.ambiguous(), loc).arg(self.ty_arg(ty));
        for n in notes {
            d = d.with_note(n);
        }
        self.emit(d);
        self.recover_err(&[cond], loc)
    }

    /// clang's note at conversion function `f` for `kind`: "conversion to
    /// integral type 'T'" (`switch`), "... declared here" (array bound),
    /// "conversion to pointer type 'T'" (`delete`).
    fn note_contextual_conversion(&self, kind: ContextualConv, f: DeclId) -> Diag {
        let to = self.contextual_conversion_type(f);
        let at = self.ast.decls.get(f).loc();
        let is_enum = self.ast.types.as_enum(self.ast.types.canonical(to)).is_some();
        let which = if is_enum { "1" } else { "0" };
        match kind {
            ContextualConv::Switch => {
                self.diag(DiagId::note_switch_conversion, at).arg(which).arg(self.ty_arg(to))
            }
            ContextualConv::ArraySize => {
                self.diag(DiagId::note_array_size_conversion, at).arg(which).arg(self.ty_arg(to))
            }
            ContextualConv::Delete => {
                self.diag(DiagId::note_delete_conversion, at).arg(self.ty_arg(to))
            }
        }
    }
}
