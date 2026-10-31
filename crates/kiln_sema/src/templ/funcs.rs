//! Function template specializations ([temp.fct.spec], [temp.inst]/5;
//! spec/sema-templates.md §6.1 step 4, §6.5): `get_function_spec` forms
//! (and memoizes by canonical arguments) the declaration of `f<args>`
//! with its substituted signature; its body is instantiated when it is
//! odr-used (`note_used_for_instantiation`), immediately for constexpr
//! and deduced-return specializations, else at the end of the
//! translation unit.

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::ids::ArgListId;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::TypeView;
use kiln_source::Loc;

use crate::Sema;
use crate::templ::args::ConvertedArgs;
use crate::templ::subst::Substituter;

impl Sema<'_, '_> {
    /// The specialization of function template `tmpl` for converted
    /// arguments `args` (§6.1 step 4): found, or created with the
    /// pattern's signature substituted. In a SFINAE context (deduction)
    /// a substitution failure is captured by the caller's frame. `Err`
    /// after a diagnostic (or a captured one); the failed specialization
    /// is memoized as invalid.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn get_function_spec(
        &mut self,
        tmpl: DeclId,
        args: &ConvertedArgs,
        loc: Loc,
    ) -> Result<DeclId, ()> {
        let tmpl = self.ast.first(tmpl);
        if let Some(s) = self.ast.templates.find_spec(tmpl, args.canonical) {
            if self.ast.decls.get(s).has(DeclFlags::INVALID) {
                return Err(());
            }
            return Ok(s);
        }
        let Some(pattern) = self.pattern_of(self.inst_template(tmpl)) else { return Err(()) };
        let kind = self.kind_of(pattern);
        if !kind.is_function() {
            return Err(());
        }
        let fspec = self.ast.decls.function(pattern).map(|x| x.spec).unwrap_or_default();
        let spec = self.builder().function_spec(
            tmpl,
            args.sugared,
            args.canonical,
            kind,
            QualType::ERROR,
            fspec,
        );
        if let Some((i, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(i, |s| {
                s.kind = SpecKind::ImplicitInstantiation;
                s.from = pattern;
                s.point_of_instantiation = loc;
            });
        }
        let Some((_, margs)) = self.spec_args(spec) else { return Err(()) };
        let ctx = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(ctx);
        let r = self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &margs);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            sub.function_spec_signature(pattern, spec)
        });
        if r.is_err() {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            return Err(());
        }
        Ok(spec)
    }

    /// Whether `f` is a function template specialization (clang's
    /// `getTemplateSpecializationInfo`).
    pub(crate) fn is_function_spec(&self, f: DeclId) -> bool {
        self.ast.decls.get(f).kind().is_function()
            && self
                .ast
                .spec_info(f)
                .is_some_and(|(_, i)| self.kind_of(i.template) == DeclKind::FunctionTemplate)
    }

    /// `f<int>`: how function template specialization `spec` (of `tmpl`
    /// with `args`) prints in an instantiation note.
    pub(crate) fn function_spec_name(&self, spec: DeclId, tmpl: DeclId, args: ArgListId) -> String {
        let _ = spec;
        self.spec_name(tmpl, args)
    }

    /// clang's template argument binding string, `[with T = int, N = 3]`,
    /// for the parameters of `tmpl` and the arguments `args`.
    pub(crate) fn template_args_desc(&self, tmpl: DeclId, args: ArgListId) -> String {
        let params = self.template_params(tmpl);
        let list = self.ast.types.args(args).to_vec();
        let mut parts = Vec::with_capacity(params.len());
        for (p, a) in params.iter().zip(list) {
            if matches!(a.view(), kiln_ast::types::ArgView::Null) {
                continue;
            }
            parts.push(format!("{} = {}", self.decl_name(*p), self.arg_str(a)));
        }
        format!("[with {}]", parts.join(", "))
    }

    /// The parameter types of `f`'s function type (`ERROR`s for a
    /// declaration without a prototype).
    pub(crate) fn proto_param_types(&self, f: DeclId) -> (Vec<QualType>, bool) {
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        // The written (sugared) prototype first, so mismatch notes print
        // the parameter as spelled; the canonical one as the fallback.
        if let TypeView::Function(p) = self.ast.types.view(ty.ty()) {
            return (self.ast.types.params(&p).to_vec(), p.info().variadic());
        }
        match self.ast.types.view(self.ast.types.canonical(ty).ty()) {
            TypeView::Function(p) => (self.ast.types.params(&p).to_vec(), p.info().variadic()),
            _ => (Vec::new(), false),
        }
    }
}

impl Sema<'_, '_> {
    /// Instantiates the definition of function template specialization
    /// `f` from the pattern's definition `pdef` (body `body`) at `poi`
    /// ([temp.inst]/5, §4.4).
    pub(crate) fn instantiate_function_spec_definition(
        &mut self,
        f: DeclId,
        pdef: DeclId,
        body: kiln_ast::StmtId,
        poi: Loc,
    ) {
        let Some((tmpl, args)) = self.spec_args(f) else { return };
        if !self.push_inst_frame(crate::templ::stack::FrameKind::FunctionSpec, f, tmpl, poi) {
            return;
        }
        if let Some((i, _)) = self.ast.spec_info(f) {
            self.ast.templates.update_spec(i, |s| {
                if s.point_of_instantiation == Loc::INVALID {
                    s.point_of_instantiation = poi;
                }
            });
        }
        let ctx = self.ast.decls.get(f).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(ctx);
        self.in_inst_context(ctx, |s| {
            s.act_on_start_function_def(f);
            let pp: Vec<DeclId> = s
                .ast
                .decls
                .function(pdef)
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default();
            let fp: Vec<DeclId> = s
                .ast
                .decls
                .function(f)
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default();
            let lbrace = s.ast.stmts.get(body).loc();
            let rbrace = if s.ast.stmts.get(body).kind() == kiln_ast::stmt::StmtKind::CompoundStmt {
                s.ast.stmts.rbrace(body)
            } else {
                lbrace
            };
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(f);
            sub.map_contexts(&pairs);
            sub.map_params(&pp, &fp);
            if sub.sema.kind_of(f) == DeclKind::CXXConstructor {
                // A constructor template's ctor-initializer, as a class
                // template member's (`templ/class.rs`).
                let (inits, colon) = sub
                    .sema
                    .inst
                    .pattern_mem_inits
                    .get(&pdef)
                    .cloned()
                    .unwrap_or((Vec::new(), lbrace));
                for info in &inits {
                    match sub.mem_initializer(info) {
                        Ok(v) => {
                            for i in &v {
                                sub.sema.act_on_mem_initializer(f, i);
                            }
                        }
                        Err(_) => sub.sema.note_failed_mem_initializer(f),
                    }
                }
                sub.sema.act_on_mem_initializers(f, &[], colon);
            }
            let b = sub.body(body);
            s.act_on_finish_function_def(f, b, rbrace);
        });
        self.pop_inst_frame();
    }
}
