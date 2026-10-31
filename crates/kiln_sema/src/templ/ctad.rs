//! Class template argument deduction ([over.match.class.deduct]) and
//! deduction-guide declarations ([temp.deduct.guide]); spec/sema.md
//! "Class template argument deduction".

use kiln_ast::decl::{DeclFlags, FnSpec};
use kiln_ast::types::AutoKeyword;
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{Chunk, DeclSpec, Declarator};

impl Sema<'_, '_> {
    /// The undeduced placeholder `auto`: the declared return type of a
    /// deduction guide before its trailing return type replaces it.
    pub fn plain_auto_type(&mut self) -> QualType {
        self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false)
    }

    /// A deduction-guide declaration for class template `template`
    /// ([temp.deduct.guide]): a `CXXDeductionGuide` named
    /// `<deduction guide for X>` in the current context, wrapped in a
    /// function template by its template head.
    pub(crate) fn act_on_deduction_guide(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
        template: DeclId,
    ) -> Option<DeclId> {
        let sloc = d.special.map_or(d.loc, |(_, l)| l);
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        let invalid = ds.invalid || d.invalid || self.is_error_type(ty);
        let Some(Chunk::Function { params, variadic, .. }) = d.chunks.first() else {
            self.templ.cur = None;
            return None;
        };
        if !self.ast.types.is_function(ty) {
            self.templ.cur = None;
            return None;
        }
        let (params, variadic) = (params.clone(), *variadic);
        let mut spec = FnSpec::new().with_variadic(variadic);
        let explicit = ds.explicit.is_some();
        let explicit_value = match (explicit, ds.explicit_expr) {
            (true, Some(e)) => self.explicit_value(e),
            (e, _) => Some(e),
        };
        if explicit && explicit_value != Some(false) {
            spec = spec.with_explicit(true);
        }
        let (ctx, lexical) = self.decl_contexts(d);
        let ctx = self.ast.definition(ctx).unwrap_or(ctx);
        let first = self.ast.first(template);
        let name = self.ast.names.deduction_guide(first);
        let (defaults, _) = self.merge_default_args(None, false, &params);
        let mut h = DeclHead::new(ctx, name, sloc);
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let f = self.builder().deduction_guide(h, ty, spec);
        if invalid {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        if explicit && let Some(e) = ds.explicit_expr {
            let _ = self.ast.decls.cold.explicit_expr.insert(f, e);
        }
        self.create_params(f, &params, &defaults);
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(f, r);
        }
        self.set_member_access(f);
        if let Some(t) = self.finish_template_pattern(f) {
            self.set_member_access(t);
        }
        Some(f)
    }

    /// Deduces the template arguments of a class template from an
    /// initializer (WP-071b): reports and fails.
    pub fn deduce_class_template_args(&self, loc: Loc) -> Option<kiln_ast::QualType> {
        self.templ_unimplemented("class template argument deduction", loc);
        None
    }
}
