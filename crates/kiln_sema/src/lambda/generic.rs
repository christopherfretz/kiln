//! Generic lambdas' conversion to pointer to function
//! ([expr.prim.lambda.closure]/9): the conversion function template's
//! specializations, deduced from the target pointer type, and the
//! call operator and invoker specializations they forward to (clang
//! `SpecializeCorrespondingLambdaCallOperatorAndInvoker`).

use kiln_ast::decl::{DeclKind, FnSpec};
use kiln_ast::types::{ExcSpec, FuncInfo, Quals, RefQual, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_source::Loc;

use crate::Sema;
use crate::templ::args::ConvertedArgs;

impl Sema<'_, '_> {
    /// The closure type whose generic lambda's conversion function
    /// template is `ft`.
    fn lambda_conversion_closure(&self, ft: DeclId) -> Option<DeclId> {
        let p = self.ast.template_info(self.ast.first(ft))?.pattern?;
        if self.kind_of(p) != DeclKind::CXXConversion
            || !self.ast.decls.get(p).has(kiln_ast::decl::DeclFlags::IMPLICIT)
        {
            return None;
        }
        let r = self.ast.decls.get(p).ctx()?;
        self.ast.closure_lambda(r).map(|_| r)
    }

    /// The return type of the function type `a` points to.
    fn pointee_fn_ret(&self, a: QualType) -> Option<QualType> {
        let TypeView::Pointer(f) = self.ast.types.canonical_view(a) else { return None };
        self.ast.types.proto(f).map(kiln_ast::types::FuncProto::ret)
    }

    /// [temp.deduct.conv] for generic lambda conversion template `ft`:
    /// `P` (`auto (*)(T...)`) with the target's return type in place of
    /// its undeduced `auto` (the specialization gets the call operator's
    /// deduced return type, [`Self::specialize_lambda_conversion`]).
    pub(crate) fn lambda_conversion_p(&mut self, ft: DeclId, p: QualType, a: QualType) -> QualType {
        if self.lambda_conversion_closure(ft).is_none() {
            return p;
        }
        let Some(ret) = self.pointee_fn_ret(a) else { return p };
        let TypeView::Pointer(f) = self.ast.types.view(p.ty()) else { return p };
        let TypeView::Function(proto) = self.ast.types.view(f.ty()) else { return p };
        let params = self.ast.types.params(&proto).to_vec();
        let exc = self.ast.types.exc_spec(&proto);
        let f = self.ast.types.function(ret, &params, proto.info(), exc);
        self.ast.types.pointer_to(f)
    }

    /// After deducing conversion template specialization `conv`:
    /// specializes the call operator for the same arguments, deduces its
    /// return type, then specializes the invoker and gives both their
    /// types (a return type other than the target's then makes the
    /// conversion sequence fail, as in clang). False if the call
    /// operator cannot be used.
    pub(crate) fn specialize_lambda_conversion(
        &mut self,
        ft: DeclId,
        conv: DeclId,
        loc: Loc,
    ) -> bool {
        let Some(closure) = self.lambda_conversion_closure(ft) else { return true };
        let Some(l) = self.ast.closure_lambda(closure) else { return true };
        let Some(op) = self.ast.exprs.lambdas.get(l).call_op else { return false };
        let Some(sugared) = self.ast.spec_info(conv).map(|(_, s)| s.args) else { return false };
        let args = ConvertedArgs { sugared, canonical: self.ast.types.canonical_args(sugared) };
        let Some(op_t) = self.ast.described_template(op) else { return false };
        let Ok(op_spec) = self.get_function_spec(op_t, &args, loc) else { return false };
        if self.check_undeduced_use(op_spec, loc) {
            return false;
        }
        let Some(fty) = self.ast.decls.get(op_spec).ty() else { return false };
        let TypeView::Function(p) = self.ast.types.view(fty.ty()) else { return false };
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let info = p.info().with_this_quals(Quals::NONE).with_ref_qual(RefQual::None);
        let invoker = self.ast.types.function(p.ret(), &params, info, exc);
        // P2564 / [expr.prim.lambda.closure]/10 per specialization: an
        // escalated call operator specialization makes this conversion
        // specialization and its invoker immediate (as `add_function_pointer_conversion`
        // does for a non-generic lambda).
        let escalated = self.ast.decls.function(op_spec).is_some_and(|x| x.spec.escalated());
        if escalated {
            self.ast.decls.mark_function(conv, FnSpec::new().with_escalated(true));
        }
        if let Some(inv) = self.ast.closure_invoker(closure)
            && let Some(inv_t) = self.ast.described_template(inv)
        {
            let Ok(inv_spec) = self.get_function_spec(inv_t, &args, loc) else { return false };
            self.ast.decls.set_ty(inv_spec, invoker);
            if escalated {
                self.ast.decls.mark_function(inv_spec, FnSpec::new().with_escalated(true));
            }
        }
        let ptr = self.ast.types.pointer_to(invoker);
        let conv_ty = self.ast.types.function(
            ptr,
            &[],
            FuncInfo::new().with_this_quals(Quals::CONST),
            ExcSpec::BasicNoexcept,
        );
        self.ast.decls.set_ty(conv, conv_ty);
        // The specialization is named by the substituted target type,
        // its placeholder return type kept (clang: `operator auto
        // (*)(int)`); mangling uses the template's name.
        let pattern_ret =
            self.ast.decls.get(op).ty().and_then(|t| match self.ast.types.view(t.ty()) {
                TypeView::Function(f) => Some(f.ret()),
                _ => None,
            });
        // (In the pattern, the placeholder is deduced as dependent.)
        let is_placeholder = |r: QualType| {
            matches!(self.ast.types.canonical_view(r), TypeView::Auto { .. })
                || self.contains_undeduced_auto(r)
        };
        let name_ret = pattern_ret.filter(|&r| is_placeholder(r)).unwrap_or(p.ret());
        let named = self.ast.types.function(name_ret, &params, info, exc);
        let named = self.ast.types.pointer_to(named);
        let canon = self.ast.types.canonical(named);
        let name = self.ast.names.conversion(canon);
        self.ast.decls.set_name(conv, name);
        true
    }
}
