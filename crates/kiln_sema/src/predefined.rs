//! Predefined identifiers `__func__`, `__FUNCTION__` and
//! `__PRETTY_FUNCTION__` ([dcl.fct.def.general]/8; clang
//! `Sema::BuildPredefinedExpr` and `PredefinedExpr::ComputeName`).

use kiln_ast::dump::AstNames;
use kiln_ast::expr::{PredefinedKind, StringKind};
use kiln_ast::strings::StringEncoding;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// A predefined identifier at `loc` (clang `BuildPredefinedExpr`):
    /// names the innermost enclosing function; outside one, warns and
    /// names the translation unit. In a dependent context the type is
    /// dependent and the name is computed at instantiation. The node
    /// records the function it names (`ExprArena::predefined_function`,
    /// spec/ast.md §16, WP-2823): the pattern's function in a template,
    /// the specialization when an instantiation rebuilds it here, none
    /// outside a function.
    pub fn act_on_predefined_expr(&mut self, kind: PredefinedKind, loc: Loc) -> ExprId {
        let func = self.enclosing_function(self.ctx);
        if func.is_none() {
            let msg = self.diag(DiagId::sema_predefined_identifier_outside_function, loc);
            self.emit(msg);
        }
        if let Some(f) = func
            && self.ctx_template_levels(f) > 0
        {
            return self.builder().predefined_expr(kind, QualType::DEPENDENT, None, Some(f), loc);
        }
        let name = match func {
            Some(f) => self.compute_predefined_name(kind, f),
            None if kind == PredefinedKind::PrettyFunction => "top level".to_owned(),
            None => String::new(),
        };
        let lit = self.ast.strings.intern(StringEncoding::Ordinary, name.as_bytes());
        let ty = crate::literal::string_literal_type(
            self,
            kiln_lex::Encoding::Ordinary,
            name.len() as u64 + 1,
        );
        let sl = self.builder().string_literal(ty, StringKind::Ordinary, lit, loc);
        self.builder().predefined_expr(kind, ty, Some(sl), func, loc)
    }

    /// The innermost function enclosing context `ctx` (clang
    /// `getPredefinedExprDecl`).
    pub(crate) fn enclosing_function(&self, ctx: DeclId) -> Option<DeclId> {
        let mut cur = Some(ctx);
        let mut guard = 0;
        while let Some(d) = cur {
            if self.ast.decls.get(d).kind().is_function() {
                return Some(d);
            }
            guard += 1;
            if guard > 1024 {
                break;
            }
            cur = self.ast.decls.get(d).ctx();
        }
        None
    }

    /// The instantiation pattern of function `f`
    /// ([`kiln_ast::Ast::function_pattern`]).
    pub(crate) fn function_pattern(&self, f: DeclId) -> DeclId {
        self.ast.function_pattern(f)
    }

    /// clang `PredefinedExpr::ComputeName(kind, f)` for function `f`.
    pub(crate) fn compute_predefined_name(&self, kind: PredefinedKind, f: DeclId) -> String {
        let ast = &self.ast;
        // Unnamed tags print with their location (`(lambda at F:L:C)`)
        // and the scopes' template arguments canonical and scoped
        // (`C<N::X>::m`), as clang prints the arguments it stores.
        let mut names = AstNames::with_sources(ast, self.diags.sm());
        names.closure_as_lambda = self.clang_compat >= 22;
        names.local_scopes = self.clang_compat >= 22;
        names.scoped_spec_args = true;
        if kind != PredefinedKind::PrettyFunction {
            let mut s = String::new();
            names.write_name(f, &mut s);
            return s;
        }
        kiln_ast::dump::pretty_function(&names, f, true)
    }
}
