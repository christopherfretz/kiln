//! Concept and variable template template parameters (P2841R7):
//! their kinds ([temp.param]/1), the template-ids that name them
//! ([temp.names]) and their substitution, spec/sema-templates.md §2.1.1.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::templates::TtpKind;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::templ::actions::TemplateIdRef;
use crate::templ::subst::{SubstFail, SubstResult, Substituter};

impl Sema<'_, '_> {
    /// The kind of template template parameter template `d` can be an
    /// argument for ([temp.arg.template]/1, /3): `None` for anything that
    /// is no template template argument.
    pub(crate) fn template_arg_kind(&self, d: DeclId) -> Option<TtpKind> {
        match self.kind_of(d) {
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate => Some(TtpKind::Type),
            DeclKind::VarTemplate => Some(TtpKind::Variable),
            DeclKind::Concept => Some(TtpKind::Concept),
            DeclKind::TemplateTemplateParm => Some(self.ttp_kind(d)),
            _ => None,
        }
    }

    /// The variable template template parameter `e` (an
    /// `UnresolvedLookupExpr`) names as `V<args>`.
    pub(crate) fn var_ttp_ref(&self, e: ExprId) -> Option<DeclId> {
        let n = self.ast.exprs.get(e);
        if n.kind() != ExprKind::UnresolvedLookupExpr {
            return None;
        }
        let l = kiln_ast::ids::ListId::new(n.b());
        let mut it = self.ast.lists.iter::<DeclId>(l);
        let p = it.next()?;
        (it.next().is_none() && self.ttp_kind(p) == TtpKind::Variable).then_some(p)
    }

    /// The kind of template template parameter `p` ([temp.param]/1);
    /// `Type` for anything else.
    pub fn ttp_kind(&self, p: DeclId) -> TtpKind {
        if self.ast.decls.get(p).kind() != DeclKind::TemplateTemplateParm {
            return TtpKind::Type;
        }
        self.ast.template_parm(p).map_or(TtpKind::Type, |x| x.ttp_kind)
    }

    /// Template template parameter `p` was declared with `concept` or
    /// `auto` (at `kw`; P2841R7 [temp.param]/1). A concept template
    /// parameter shall not have associated constraints ([temp.param]/5):
    /// a requires-clause or a type-constraint in its head is an error
    /// (kiln; clang 22 accepts both, spec/sema-templates.md §2.1.1).
    pub fn act_on_ttp_kind(&mut self, p: DeclId, kind: TtpKind, kw: Loc) {
        self.builder().set_ttp_kind(p, kind);
        if kind != TtpKind::Concept {
            return;
        }
        let Some(list) = self.ast.template_parm(p).and_then(|x| x.params) else { return };
        let l = *self.ast.templates.param_list(list);
        let constrained = l.requires.is_some()
            || self
                .ast
                .lists
                .iter::<DeclId>(l.params)
                .any(|q| self.ast.template_parm(q).is_some_and(|x| x.constraint.is_some()));
        if constrained {
            self.error(DiagId::sema_concept_template_param_constraints, kw);
        }
    }
}

impl Substituter<'_, '_, '_> {
    /// `P<args>` (expression `e`, at `loc`) for concept or variable
    /// template template parameter `p`: the template-id of `p`'s
    /// argument with the substituted arguments ([temp.names]; a concept-id
    /// is checked, [temp.constr.concept]/2, and a variable template
    /// specialization named). `None` when `p` is not such a parameter.
    pub(crate) fn ttp_template_id(
        &mut self,
        e: ExprId,
        p: DeclId,
        loc: Loc,
    ) -> Option<SubstResult<ExprId>> {
        if self.sema.ttp_kind(p) == TtpKind::Type {
            return None;
        }
        Some(self.ttp_template_id_of(e, p, loc))
    }

    fn ttp_template_id_of(&mut self, e: ExprId, p: DeclId, loc: Loc) -> SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(e);
        let list = if n.kind() == ExprKind::ConceptSpecializationExpr {
            <kiln_ast::ids::ArgListId as kiln_ast::ids::RawId>::from_raw(n.b())
        } else {
            self.sema.ast.templates.expr_targs.get(e).copied()
        };
        let list = list.ok_or(SubstFail)?;
        let at = n.loc();
        let tn = self.sema.template_name_for(p);
        let tn = self.template_name(tn, loc)?;
        let nl = self.args(list, loc)?;
        let args = self.sema.ast.types.args(nl).to_vec();
        match self.sema.act_template_id(tn, &args, at, at) {
            Ok(TemplateIdRef::Value(x)) => Ok(x),
            _ => Err(SubstFail),
        }
    }
}
