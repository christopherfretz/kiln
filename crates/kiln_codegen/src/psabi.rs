//! `-Wpsabi`: clang `X86_64TargetCodeGenInfo::checkFunctionCallABI`
//! (spec/codegen.md "Calls").

use kiln_ast::expr::ExprKind;
use kiln_ast::types::{TypeView, vector_diag_arg};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::{Loc, Span};

use crate::module::Cg;

impl Cg<'_> {
    /// clang `checkFunctionCallABI` on x86-64, at the location of call
    /// `e` to `callee` with arguments `args`: passing or returning a
    /// vector wider than 128 bits depends on the `avx` (wider than 256
    /// bits: `avx512f`) target feature. When neither the caller nor the
    /// callee has it (their `-march=` / `-m<feature>` / `target("...")`
    /// features), that is a warning; when only one has it, the error
    /// `err_avx_calling_convention` (WP-1000); when both do, nothing. The
    /// first argument diagnosed ends the check; otherwise the return type
    /// is checked.
    ///
    /// `this_arg`: the call passes an object argument first, which clang
    /// counts when it picks the parameter whose type it prints (a
    /// member's second parameter for its first argument). clang 22 does
    /// not warn for a callee that is not externally visible (the error
    /// stands).
    pub(crate) fn psabi_call_check(
        &mut self,
        e: ExprId,
        callee: DeclId,
        args: &[ExprId],
        this_arg: bool,
    ) {
        if !self.is_x86() {
            return;
        }
        let ast = self.ast;
        let Some(fty) = ast.decls.get(callee).ty() else { return };
        // The function type as declared (`proto` is canonical): clang
        // prints the declared parameter and return types.
        let TypeView::Function(p) = ast.types.view(ast.types.desugar_top(fty).ty()) else {
            return;
        };
        let params = ast.types.params(&p).to_vec();
        let skip = usize::from(this_arg);
        let span = Span::empty(self.call_loc(e));
        for (i, &a) in args.iter().enumerate() {
            // clang's `CallArg` type: the prototype's parameter, or the
            // argument passed through `...`.
            let ty = params.get(i).copied().unwrap_or_else(|| ast.exprs.get(a).ty());
            if let Some(f) = self.avx_feature(ty) {
                let shown = params.get(i + skip).copied().unwrap_or(ty);
                if self.check_avx(span, callee, true, shown, f) {
                    return;
                }
            }
        }
        if let Some(f) = self.avx_feature(p.ret()) {
            self.check_avx(span, callee, false, p.ret(), f);
        }
    }

    /// clang `checkAVXParam`: whether a diagnostic was given for a value
    /// of type `q` needing `feature` in the call to `callee`.
    fn check_avx(
        &mut self,
        span: Span,
        callee: DeclId,
        argument: bool,
        q: QualType,
        feature: &str,
    ) -> bool {
        let has = |m: &crate::target_features::FeatureMap| m.get(feature) == Some(&true);
        let caller = has(&self.feature_map(self.f.decl));
        let callee_has = has(&self.feature_map(Some(callee)));
        let id = match (caller, callee_has) {
            (true, true) => return false,
            (false, false) if self.opts.clang_compat >= 22 && self.fn_internal(callee) => {
                return false;
            }
            (false, false) => DiagId::cg_avx_calling_convention,
            _ => DiagId::cg_avx_calling_convention_mismatch,
        };
        let ast = self.ast;
        let shown =
            vector_diag_arg(&ast.types, ast, q, self.opts.clang_compat >= 22).unwrap_or_default();
        let d = Diag::new(id, span).arg(u8::from(argument)).arg(shown).arg(feature.to_owned());
        self.diags.push(d);
        true
    }

    /// clang `getExprLoc` of call `e`: a member call's member name, an
    /// overloaded binary, prefix or postfix operator's token except
    /// `->`, `()`, `[]`, `co_await`, `++` and `--`, which begin at their first
    /// operand like a plain call; a construction's own location.
    fn call_loc(&self, e: ExprId) -> Loc {
        use kiln_ast::names::OverloadedOperator as O;
        let ast = self.ast;
        let n = ast.exprs.get(e);
        match n.kind() {
            ExprKind::CallExpr => ast.begin_loc(e),
            ExprKind::CXXMemberCallExpr => {
                let c = ast.exprs.callee(e);
                match ast.exprs.get(c).kind() {
                    ExprKind::MemberExpr => ast.exprs.get(c).loc(),
                    _ => ast.begin_loc(e),
                }
            }
            ExprKind::CXXOperatorCallExpr
                if matches!(
                    ast.exprs.operator_call_op(e),
                    O::Arrow | O::Call | O::Subscript | O::Coawait | O::PlusPlus | O::MinusMinus
                ) =>
            {
                ast.begin_loc(e)
            }
            _ => n.loc(),
        }
    }

    /// The feature that changes how a value of type `q` is passed: a
    /// vector wider than 256 bits needs `avx512f`, wider than 128 `avx`.
    fn avx_feature(&mut self, q: QualType) -> Option<&'static str> {
        if !matches!(self.ast.types.canonical_view(q), TypeView::Vector { .. }) {
            return None;
        }
        match self.size_align(q).0 * 8 {
            ..=128 => None,
            129..=256 => Some("avx"),
            _ => Some("avx512f"),
        }
    }
}
