//! Substitution into contract assertions (spec/contracts.md §5.6): a
//! `contract_assert` statement of an instantiated body, and the
//! function contract assertions of an instantiated definition, rebuilt
//! through the parser's actions so their checks run on the
//! instantiation.

use kiln_ast::stmt::ContractKind;
use kiln_ast::{DeclId, StmtId};

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
