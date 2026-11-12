//! clang's `diagnose_if` attribute (WP-486; spec/sema.md "The
//! `diagnose_if` attribute"): `__attribute__((diagnose_if(cond, "message",
//! "error"|"warning")))` on a function emits `message` as an error or a
//! `-Wuser-defined-warnings` warning where the function is used and
//! `cond`, evaluated as a constant expression, is true. A condition that
//! names a parameter or `this` is *argument-dependent* and is evaluated at
//! each call with the call's converted arguments (clang
//! `diagnoseArgDependentDiagnoseIfAttrs`); any other is evaluated at each
//! use of the function's name (clang `diagnoseArgIndependentDiagnoseIfAttrs`).
//! A condition that is not a constant expression is ignored.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::{DiagId, Group};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::decl::AttrArg;

/// `diagnose_if` flag (bit 2): the diagnostic type is `"error"`.
pub(crate) const DIAGNOSE_IF_ERROR: u8 = 4;
/// `diagnose_if` flag (bit 4): the condition names a parameter or `this`.
pub(crate) const DIAGNOSE_IF_ARG_DEPENDENT: u8 = 16;
/// `diagnose_if` flag (bit 5): written with a warning group (the fourth
/// argument, clang 22), so the diagnostic has no "from `diagnose_if`" note.
pub(crate) const DIAGNOSE_IF_GROUP: u8 = 32;

impl Sema<'_, '_> {
    /// `diagnose_if(cond, "message", "type"[, "group"])` as written (clang
    /// `handleDiagnoseIfAttr` / `checkFunctionConditionAttr`): `a` is the
    /// condition, contextually converted to `bool` unless it is
    /// type-dependent, `b` the message's string literal; the type is
    /// `"error"` or `"warning"`. clang 22 takes an optional fourth
    /// argument, a warning group name; clang 19 exactly three. The parser
    /// parses the arguments with the function's parameters in scope.
    pub(crate) fn diagnose_if_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let name = "'diagnose_if'";
        let arity = if self.clang_compat >= 22 {
            if args.len() < 3 {
                Some((DiagId::sema_attribute_too_few_arguments, "3"))
            } else if args.len() > 4 {
                Some((DiagId::sema_attribute_too_many_arguments, "4"))
            } else {
                None
            }
        } else {
            (args.len() != 3).then_some((DiagId::sema_attribute_wrong_number_arguments, "3"))
        };
        if let Some((id, n)) = arity {
            let d = self.diag(id, loc).arg(name).arg(n);
            self.emit(d);
            return None;
        }
        let mut exprs = Vec::with_capacity(args.len());
        for &a in args {
            let AttrArg::Expr(e) = a else {
                self.error(DiagId::parse_expected_expression, loc);
                return None;
            };
            exprs.push(e);
        }
        let cond = exprs[0];
        let cond = if self.expr_is_type_dependent(cond) {
            cond
        } else {
            match conv::contextually_convert_to_bool(self, cond) {
                Ok(c) => c,
                Err(err) => {
                    self.emit_conv_error(err, self.ast.exprs.get(cond).loc());
                    return None;
                }
            }
        };
        let cond = self.act_on_constant_full_expr(cond);
        let requires_string = |s: &mut Self, e: ExprId| {
            let at = s.ast.exprs.get(e).loc();
            let d = s.diag(DiagId::parse_attribute_requires_string, at).arg("diagnose_if");
            s.emit(d);
        };
        let Some(msg) = self.string_lit_of(exprs[1]) else {
            requires_string(self, exprs[1]);
            return None;
        };
        let Some(ty) = self.string_text(exprs[2]) else {
            requires_string(self, exprs[2]);
            return None;
        };
        let mut flags = match ty.as_str() {
            "error" => DIAGNOSE_IF_ERROR,
            "warning" => 0,
            _ => {
                let at = self.ast.exprs.get(exprs[2]).loc();
                self.error(DiagId::sema_diagnose_if_invalid_diagnostic_type, at);
                return None;
            }
        };
        if let Some(&g) = exprs.get(3) {
            let Some(group) = self.string_text(g) else {
                requires_string(self, g);
                return None;
            };
            if !Group::is_known_name(&group) {
                let at = self.ast.exprs.get(g).loc();
                let d = self.diag(DiagId::sema_unknown_warning_group, at).arg(group);
                self.emit(d);
                return None;
            }
            flags |= DIAGNOSE_IF_GROUP;
        }
        if self.names_param_or_this(cond, 0) {
            flags |= DIAGNOSE_IF_ARG_DEPENDENT;
        }
        let node = AttrNode {
            kind: AttrKind::DIAGNOSE_IF,
            syntax,
            flags,
            loc,
            a: cond.raw(),
            b: msg.raw(),
        };
        self.any_diagnose_if = true;
        Some(self.ast.attrs.push(node))
    }

    /// Whether `e` names a function parameter or `this` (clang
    /// `ArgumentDependenceChecker`).
    pub(crate) fn names_param_or_this(&self, e: ExprId, depth: u32) -> bool {
        if depth > 256 {
            return false;
        }
        let ex = &self.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::CXXThisExpr => true,
            ExprKind::DeclRefExpr => self.kind_of(ex.decl(e)) == DeclKind::ParmVar,
            _ => ex
                .children(e, &self.ast.lists)
                .into_iter()
                .any(|c| self.names_param_or_this(c, depth + 1)),
        }
    }

    /// The `diagnose_if` attributes of function `f` whose condition is
    /// (`dependent`) or is not argument-dependent, over its declarations
    /// oldest first (clang merges a redeclaration's onto the previous
    /// ones), each with the declaration it is written on.
    pub(crate) fn diagnose_if_attrs(&self, f: DeclId, dependent: bool) -> Vec<(DeclId, AttrNode)> {
        let decls: Vec<DeclId> = self.ast.redecls_of(f).take(4096).collect();
        let mut out = Vec::new();
        for &d in decls.iter().rev() {
            for a in self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(d)) {
                let n = *self.ast.attrs.get(a);
                if n.kind == AttrKind::DIAGNOSE_IF
                    && n.flags & 2 == 0
                    && (n.flags & DIAGNOSE_IF_ARG_DEPENDENT != 0) == dependent
                {
                    out.push((d, n));
                }
            }
        }
        out
    }

    /// The argument-dependent `diagnose_if` attributes of function `f`
    /// called with the converted arguments `args` (default arguments
    /// included, the implicit object argument not), diagnosed at `loc`
    /// (clang `diagnoseArgDependentDiagnoseIfAttrs`, from `checkCall`).
    /// Not in a template definition: each instantiation checks its calls,
    /// those it shares with the definition through `diagnose_if_recheck`
    /// ([`Self::recheck_diagnose_if`]).
    pub(crate) fn check_diagnose_if_call(&mut self, f: DeclId, args: &[ExprId], loc: Loc) {
        if !self.any_diagnose_if {
            return;
        }
        let attrs = self.diagnose_if_attrs(f, true);
        if attrs.is_empty() {
            return;
        }
        if self.in_template() {
            if let Some(&first) = args.first()
                && !args.iter().any(|&a| self.expr_is_value_dependent(a))
            {
                self.diagnose_if_recheck.insert(first, (f, args.to_vec(), loc));
            }
            return;
        }
        let args: Vec<Option<ExprId>> = args.iter().copied().map(Some).collect();
        self.emit_diagnose_if(
            f,
            &attrs,
            |s, holder, cond| s.cond_with_args(holder, cond, &args, 0, loc),
            loc,
        );
    }

    /// The call of the template definition whose first converted argument
    /// is `first`, shared by an instantiation: checked now, as clang checks
    /// its rebuilt call ([`Self::check_diagnose_if_call`]).
    pub(crate) fn recheck_diagnose_if(&mut self, first: ExprId) {
        if let Some((f, args, loc)) = self.diagnose_if_recheck.get(&first).cloned() {
            self.check_diagnose_if_call(f, &args, loc);
        }
    }

    /// The argument-independent `diagnose_if` attributes of `d`, used at
    /// `loc` (clang `diagnoseArgIndependentDiagnoseIfAttrs`, from
    /// `DiagnoseUseOfDecl`).
    pub(crate) fn diagnose_if_use(&mut self, d: DeclId, loc: Loc) {
        if !self.any_diagnose_if || self.ast.decls.function(d).is_none() {
            return;
        }
        let attrs = self.diagnose_if_attrs(d, false);
        if attrs.is_empty() {
            return;
        }
        self.emit_diagnose_if(
            d,
            &attrs,
            |s, _, cond| {
                if s.expr_is_value_dependent(cond) {
                    return None;
                }
                s.eval_with_params(Some(cond), &[], cond, loc).and_then(|v| s.const_truth(v))
            },
            loc,
        );
    }

    /// clang `diagnoseDiagnoseIfAttrsWith`: the first `"error"` attribute
    /// among `attrs` whose condition is true is diagnosed alone; failing
    /// one, every `"warning"` one whose condition is true, in order.
    fn emit_diagnose_if(
        &mut self,
        f: DeclId,
        attrs: &[(DeclId, AttrNode)],
        mut holds: impl FnMut(&mut Self, DeclId, ExprId) -> Option<bool>,
        loc: Loc,
    ) {
        let mut check = |s: &mut Self, error: bool| -> Vec<AttrNode> {
            let mut hits = Vec::new();
            for &(holder, n) in attrs {
                if (n.flags & DIAGNOSE_IF_ERROR != 0) != error {
                    continue;
                }
                let Some(cond) = ExprId::from_raw(n.a) else { continue };
                if holds(s, holder, cond) == Some(true) {
                    hits.push(n);
                    if error {
                        break;
                    }
                }
            }
            hits
        };
        let mut hits = check(self, true);
        if hits.is_empty() {
            hits = check(self, false);
        }
        for n in hits {
            let text = ExprId::from_raw(n.b).and_then(|m| self.string_msg(m)).unwrap_or_default();
            let id = if n.flags & DIAGNOSE_IF_ERROR != 0 {
                DiagId::sema_diagnose_if_succeeded_error
            } else {
                DiagId::sema_diagnose_if_succeeded_warning
            };
            // clang reports each one, at the same location as the others.
            let mut d = self.diag(id, loc).arg(text).allow_duplicate();
            if n.flags & DIAGNOSE_IF_GROUP == 0 {
                let note = self
                    .diag(DiagId::note_from_diagnose_if, n.loc)
                    .arg(self.name_for_diagnostic(f));
                d = d.with_note(note);
            }
            self.emit(d);
        }
    }
}
