//! [temp.inst]/12: the default arguments of an instantiated function are
//! instantiated when a call uses them, not with the function's
//! declaration (spec/sema-templates.md §4.4; clang
//! `InstantiateDefaultArgument`). Substitution records them in
//! `InstState::lazy_defaults` (`subst/decls.rs` `defer_default_args`);
//! a call that needs one substitutes it here, with the arguments of the
//! specialization the function belongs to, under a
//! "default function argument" instantiation frame.

use kiln_ast::DeclId;
use kiln_ast::expr::ExprKind;
use kiln_source::Loc;

use crate::Sema;
use crate::templ::stack::FrameKind;
use crate::templ::subst::Substituter;

impl Sema<'_, '_> {
    /// Whether `f` is declared in a local scope: some enclosing context
    /// is a function (a member of a local class, a lambda's call
    /// operator; clang `isInLocalScopeForInstantiation`).
    pub(crate) fn decl_in_local_scope(&self, f: DeclId) -> bool {
        let mut c = self.ast.decls.get(f).ctx();
        while let Some(x) = c {
            if self.ast.decls.function(x).is_some() {
                return true;
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// Instantiates the default argument of parameter `p` of `f` if it
    /// was deferred, for a use at `poi`. Returns `false` when the
    /// instantiation failed with an error (the caller then reports
    /// nothing more, as clang drops the call).
    pub(crate) fn instantiate_lazy_default(&mut self, f: DeclId, p: DeclId, poi: Loc) -> bool {
        let Some((pat, a)) = self.inst.lazy_defaults.remove(&p) else { return true };
        if self.ast.decls.param(p).is_some_and(|x| x.default_arg.is_some()) {
            return true;
        }
        let _ = self.instantiate_lazy_default_inner(f, p, pat, a, poi);
        let ok = self.ast.decls.param(p).and_then(|x| x.default_arg).is_some_and(|e| {
            self.ast.exprs.get(e).kind() != ExprKind::RecoveryExpr && !self.is_err_expr(e)
        });
        if !ok {
            // A later use instantiates (and reports) it again, as in clang.
            self.ast.decls.reset_failed_default_arg(p);
            self.inst.lazy_defaults.insert(p, (pat, a));
        }
        ok
    }

    fn instantiate_lazy_default_inner(
        &mut self,
        f: DeclId,
        p: DeclId,
        pat: DeclId,
        a: DeclId,
        poi: Loc,
    ) -> bool {
        // The specialization whose arguments the default is substituted
        // with: `f` itself (a function template specialization), or the
        // class template specialization `f` is a member of.
        let (entity, args, pairs, frame_args) = if self.is_function_spec(f) {
            let Some((tmpl, args)) = self.spec_args(f) else { return false };
            let pairs = self.inst_context_pairs(self.spec_pairs_ctx(f, tmpl));
            let fa = self.ast.spec_info(f).map(|(_, i)| i.args);
            (f, args, pairs, fa)
        } else {
            let Some(spec) = self.inst_owner(f) else { return false };
            let Some((tmpl, args)) = self.spec_args(spec) else { return false };
            let Some(pclass) = self.spec_pattern_class(spec, tmpl) else { return false };
            let mut pairs = self.inst_context_pairs(spec);
            if pairs.first().is_none_or(|&(_, x)| x != spec) {
                pairs.insert(0, (pclass, spec));
            }
            let fa = self.ast.spec_info(spec).map(|(_, i)| i.args);
            (spec, args, pairs, fa)
        };
        if !self.push_inst_frame(FrameKind::DefaultFunctionArg, f, pat, poi) {
            return false;
        }
        if let Some(l) = frame_args {
            self.set_frame_args(l);
        }
        let ctx = self.ast.decls.get(f).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(entity);
            sub.map_contexts(&pairs);
            sub.lazy_default_arg(pat, a, f, p);
        });
        self.pop_inst_frame();
        true
    }
}
