//! Substitution into contract assertions (spec/contracts.md §5.6): a
//! `contract_assert` statement of an instantiated body, and the
//! function contract assertions of an instantiated definition, rebuilt
//! through the parser's actions so their checks run on the
//! instantiation.

use kiln_ast::stmt::ContractKind;
use kiln_ast::{DeclId, StmtId};
use kiln_source::Loc;

use super::{DeclOrPack, SubstFail, SubstResult, Substituter};

impl Substituter<'_, '_, '_> {
    /// Substitutes the contract assertion `s` of the function `f` being
    /// instantiated (`None`: a `contract_assert` statement).
    pub(crate) fn contract_stmt(&mut self, s: StmtId, f: Option<DeclId>) -> SubstResult<StmtId> {
        let st = &self.sema.ast.stmts;
        let (kind, cond, result) =
            (st.contract_kind(s), st.contract_cond(s), st.contract_result_name(s));
        let loc = st.get(s).loc();
        let named = result.and_then(|r| {
            let d = self.sema.ast.decls.get(r);
            d.name().as_ident().map(|n| (r, n, d.loc()))
        });
        let sc = if kind == ContractKind::Assert {
            self.sema.act_on_start_contract_assert()
        } else {
            self.sema.act_on_start_contract(kind, f, named.map(|(_, n, l)| (n, l)), true)
        };
        if let (Some((r, _, _)), Some(nr)) = (named, sc.result_name()) {
            self.locals.insert(r, DeclOrPack::Decl(nr));
        }
        let c = self.expr(cond).ok();
        let failed = c.is_none();
        let out = self.sema.act_on_end_contract(sc, c, loc);
        if failed { Err(SubstFail) } else { Ok(out) }
    }

    /// Instantiates the function contract assertions of pattern
    /// definition `pdef` (or of the earlier declaration of the pattern
    /// that carries them) for the specialization `f`, whose parameters
    /// are in scope. A failed assertion is dropped and makes `f` invalid.
    pub(crate) fn function_contracts(&mut self, f: DeclId, pdef: DeclId) {
        let Some((src, stmts)) = self.sema.pattern_contracts(pdef) else { return };
        if src != pdef {
            let params = |s: &Self, d: DeclId| -> Vec<DeclId> {
                s.sema
                    .ast
                    .decls
                    .function(d)
                    .map(|x| s.sema.ast.lists.iter::<DeclId>(x.params).collect())
                    .unwrap_or_default()
            };
            let (sp, fp) = (params(self, src), params(self, f));
            self.map_params(&sp, &fp);
        }
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            match self.contract_stmt(s, Some(f)) {
                Ok(x) => out.push(x),
                Err(_) => self.stmt_failed = true,
            }
        }
        self.sema.install_instantiated_contracts(f, &out);
    }
}

impl crate::Sema<'_, '_> {
    /// The function contract assertions of the specialization `f` whose
    /// pattern `pattern` has no definition in the translation unit (or
    /// whose definition an explicit instantiation declaration keeps out
    /// of it), instantiated without a body so that a caller-side check
    /// (`-fcontracts-client-check=`, spec/contracts.md §6.1) finds them
    /// (WP-1520; GCC 16.1 checks a structured binding's `get` so, and
    /// fails on a call with arguments). Only under a client check:
    /// otherwise nothing reads them, and the declaration is left as the
    /// standard leaves it ([temp.inst]/5).
    pub(crate) fn instantiate_declared_contracts(&mut self, f: DeclId, pattern: DeclId, poi: Loc) {
        use crate::templ::stack::FrameKind;
        if self.contract_checks.client == kiln_ast::stmt::ClientCheck::None
            || self.ast.decls.cold.contracts.contains(f)
            || self.pattern_contracts(pattern).is_none()
        {
            return;
        }
        let spec = self.is_function_spec(f);
        let (args, entity, pairs, kind, from) = if spec {
            let Some((tmpl, args)) = self.spec_args(f) else { return };
            let pairs = self.inst_context_pairs(self.spec_pairs_ctx(f, tmpl));
            (args, f, pairs, FrameKind::FunctionSpec, tmpl)
        } else {
            let Some(owner) = self.inst_owner(f) else { return };
            let Some((tmpl, args)) = self.spec_args(owner) else { return };
            let Some(pclass) = self.spec_pattern_class(owner, tmpl) else { return };
            let mut pairs = self.inst_context_pairs(owner);
            if pairs.first().is_none_or(|&(_, x)| x != owner) {
                pairs.insert(0, (pclass, owner));
            }
            (args, owner, pairs, FrameKind::MemberFunction, pattern)
        };
        if self.inst.stack.contains(kind, f) || !self.push_inst_frame(kind, f, from, poi) {
            return;
        }
        let ctx = self.ast.decls.get(f).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(ctx, |s| {
            let params = |s: &Self, d: DeclId| -> Vec<DeclId> {
                s.ast
                    .decls
                    .function(d)
                    .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                    .unwrap_or_default()
            };
            let (pp, fp) = (params(s, pattern), params(s, f));
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(entity);
            sub.map_contexts(&pairs);
            sub.map_params(&pp, &fp);
            sub.function_contracts(f, pattern);
        });
        self.pop_inst_frame();
    }
}
