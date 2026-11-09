//! [`TypeNames`] over a [`Sema`]'s AST: how declarations, expressions and
//! values print inside types in diagnostics.
//!
//! A subset of the parser package's (WP-048) printer of the same name;
//! on merge, that one is kept.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::types::TypeNames;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType};
use kiln_source::symbol::Symbol;

use crate::Sema;

/// Prints names from an AST.
#[derive(Debug, Clone, Copy)]
pub struct SemaNames<'s> {
    ast: &'s Ast,
    /// Where unnamed tags' locations resolve (`(lambda at F:L:C)`).
    sources: Option<&'s kiln_source::SourceManager>,
    /// A closure type in a scope prints as `(lambda)` (clang 22:
    /// `'f()::(lambda)::operator()<int>'`) rather than `(anonymous class)`
    /// (clang 19).
    closure_as_lambda: bool,
    /// [`TypeNames::clang_compat`]: `Sema::clang_compat` (19 without a
    /// `Sema`).
    clang_compat: u32,
    /// A function template specialization whose own arguments print with
    /// their types ([`Self::typed_args_of`]).
    typed_args: Option<DeclId>,
}

impl<'s> SemaNames<'s> {
    /// Names for `sema`'s AST.
    pub fn new(sema: &'s Sema<'_, '_>) -> Self {
        SemaNames {
            ast: &sema.ast,
            sources: Some(sema.diags.sm()),
            closure_as_lambda: sema.clang_compat >= 22,
            clang_compat: sema.clang_compat,
            typed_args: None,
        }
    }

    /// Names for `ast` alone.
    pub fn of_ast(ast: &'s Ast) -> Self {
        SemaNames {
            ast,
            sources: None,
            closure_as_lambda: false,
            clang_compat: 22,
            typed_args: None,
        }
    }

    /// These names, printing the template arguments of function template
    /// specialization `f` with their types (`'f<2U>'`, `'g<3UL, int>'`):
    /// clang's `FunctionDecl::getNameForDiagnostic` prints them without
    /// the template parameter list, so `shouldIncludeTypeForArgument` is
    /// true for each (spec/diag.md §5). Nested template-ids keep their
    /// own rule (`'f<X<2>>'`).
    #[must_use]
    pub fn typed_args_of(self, f: DeclId) -> Self {
        SemaNames { typed_args: Some(f), ..self }
    }

    /// These names, printing a closure type in a scope as `(lambda)` at
    /// every `-fclang-compat` level (kiln's own dumps, which no clang
    /// prints: spec/sema.md §7.5.1).
    #[must_use]
    pub fn closures_as_lambda(self) -> Self {
        SemaNames { closure_as_lambda: true, ..self }
    }

    /// An explicit class template specialization named as a lookup
    /// context ("no type named 'x' in '`std::hash<monostate>`'"): clang's
    /// `TypePrinter::printTag` prints its arguments as written in the
    /// specialization (`getTypeAsWritten`), so a type argument is not
    /// scoped ([temp.expl.spec]/1). `None` for any other declaration.
    pub fn explicit_spec_context(&self, d: DeclId) -> Option<String> {
        let (_, info) = self.ast.spec_info(d)?;
        if info.kind != kiln_ast::templates::SpecKind::ExplicitSpecialization {
            return None;
        }
        let mut s = String::new();
        self.write_decl_name(d, true, &mut s);
        kiln_ast::types::write_arg_list_of(&self.ast.types, self, d, info.print_args, &mut s);
        Some(s)
    }

    fn simple(&self, d: DeclId) -> Option<String> {
        let name = self.ast.decls.get(d).name();
        let s = self.ast.names.simple_spelling(name, self.ast.symbols()).unwrap_or_else(|| {
            // Constructor, destructor and conversion-function names
            // (`X`, `~X`, `operator int`, a conversion function template
            // specialization by its substituted target type).
            let mut s = String::new();
            kiln_ast::dump::AstNames::new(self.ast).write_name(d, &mut s);
            s
        });
        (!s.is_empty()).then_some(s)
    }

    fn write_unscoped(&self, d: DeclId, out: &mut String) {
        if let Some(s) = self.simple(d) {
            out.push_str(&s);
        } else {
            // `(unnamed struct at F:L:C)`, `(lambda at F:L:C)`, ...
            let names = match self.sources {
                Some(sm) => kiln_ast::dump::AstNames::with_sources(self.ast, sm),
                None => kiln_ast::dump::AstNames::new(self.ast),
            };
            names.write_decl_name(d, false, out);
        }
    }

    /// clang `NamespaceDecl::isRedundantInlineQualifierFor`: inline
    /// namespace `ns` can be left out of a scope printed before
    /// `name_in_scope` when the enclosing (non-transparent) context's
    /// lookup table has as many entries for that name as `ns`'s own
    /// (the inline namespace's names are entered in its parents too).
    fn redundant_inline(&self, ns: DeclId, name_in_scope: kiln_ast::names::DeclName) -> bool {
        if name_in_scope.is_empty()
            || !self
                .ast
                .decls
                .namespace(self.ast.first(ns))
                .is_some_and(kiln_ast::decl::NamespaceData::is_inline)
        {
            return false;
        }
        let mut parent = self.ast.decls.get(ns).ctx();
        while let Some(p) = parent
            && matches!(self.ast.decls.get(p).kind(), DeclKind::LinkageSpec | DeclKind::Export)
        {
            parent = self.ast.decls.get(p).ctx();
        }
        let Some(parent) = parent else { return false };
        self.ast.lookup_local(ns, name_in_scope).len()
            == self.ast.lookup_local(parent, name_in_scope).len()
    }

    fn write_scope(&self, d: DeclId, out: &mut String) {
        let mut chain = Vec::new();
        let mut name_in_scope = self.ast.decls.get(d).name();
        let mut cur = self.ast.decls.get(d).ctx();
        // clang `TypePrinter::AppendScope` stops at the first function
        // scope: a local type prints without the scope of the function
        // that holds it (`(unnamed enum at …)` in `A::k`, not `A::…`).
        let is_type = matches!(self.ast.decls.get(d).kind(), DeclKind::CXXRecord | DeclKind::Enum);
        let is_enumerator = self.ast.decls.get(d).kind() == DeclKind::EnumConstant;
        let own_ctx = cur;
        while let Some(c) = cur {
            let n = self.ast.decls.get(c);
            if is_type && (n.kind().is_function() || n.kind() == DeclKind::FunctionTemplate) {
                break;
            }
            match n.kind() {
                DeclKind::Namespace if self.redundant_inline(c, name_in_scope) => {}
                DeclKind::Namespace => {
                    chain.push(self.simple(c).unwrap_or_else(|| "(anonymous namespace)".into()));
                    name_in_scope = n.name();
                }
                // [dcl.enum]/11: an unscoped enumerator is declared in the
                // scope enclosing its enumeration, so clang's
                // `printNestedNameSpecifier` skips an unscoped enumeration
                // (`S::m`, not `S::(unnamed enum ...)::m`; WP-929). clang 19
                // skips only an enumerator's own enumeration (a type defined
                // in an enumerator's initializer is `E::S`); clang 22 skips
                // every unscoped one (`S`).
                DeclKind::Enum
                    if (self.clang_compat >= 22 || (is_enumerator && cur == own_ctx))
                        && !self
                            .ast
                            .decls
                            .enumeration(self.ast.definition(c).unwrap_or(c))
                            .is_some_and(kiln_ast::decl::EnumData::is_scoped) => {}
                DeclKind::CXXRecord | DeclKind::Enum => {
                    let mut s = String::new();
                    self.write_unscoped(c, &mut s);
                    if let Some(args) = self.ast.spec_print_args(c) {
                        // clang `AppendScope` / `printNestedNameSpecifier`
                        // print a specialization's canonical arguments,
                        // which carry no sugar to suppress their scope
                        // (`std::Ptr<std::D<int>>::g`, WP-172).
                        kiln_ast::types::write_arg_list_scoped_of(
                            &self.ast.types,
                            self,
                            c,
                            args,
                            &mut s,
                        );
                    }
                    chain.push(s);
                    name_in_scope = n.name();
                }
                _ => {}
            }
            cur = n.ctx();
        }
        for c in chain.iter().rev() {
            out.push_str(c);
            out.push_str("::");
        }
    }
}

impl SemaNames<'_> {
    /// Class `d` declared inside a function, as clang's
    /// `NamedDecl::printQualifiedName` prints it: unqualified directly in
    /// the function, else scoped through it (`f()::L::N`). `None` outside
    /// a function.
    pub(crate) fn local_class_decl_name(&self, d: DeclId) -> Option<String> {
        if !self.in_local_scope(d) {
            return None;
        }
        let mut s = String::new();
        let in_fn = self.ast.decls.get(d).ctx().is_some_and(|c| {
            self.ast.decls.get(c).kind().is_function() || self.ast.closure_lambda(c).is_some()
        });
        if !in_fn {
            self.write_decl_scope(d, &mut s);
        }
        self.write_unscoped(d, &mut s);
        Some(s)
    }

    /// Whether a function or closure type encloses `d`.
    fn in_local_scope(&self, d: DeclId) -> bool {
        let mut cur = self.ast.decls.get(d).ctx();
        while let Some(c) = cur {
            if self.ast.decls.get(c).kind().is_function() || self.ast.closure_lambda(c).is_some() {
                return true;
            }
            cur = self.ast.decls.get(c).ctx();
        }
        false
    }

    /// The scope of function `d` as clang's
    /// `NamedDecl::printNestedNameSpecifier` prints it: an enclosing
    /// function as `f(int)`, a closure type as `(anonymous class)`
    /// (`(lambda)` under clang 22 compatibility).
    fn write_decl_scope(&self, d: DeclId, out: &mut String) {
        let mut chain = Vec::new();
        let mut name_in_scope = self.ast.decls.get(d).name();
        let mut cur = self.ast.decls.get(d).ctx();
        while let Some(c) = cur {
            let n = self.ast.decls.get(c);
            let k = n.kind();
            if k == DeclKind::Namespace && self.redundant_inline(c, name_in_scope) {
                cur = n.ctx();
                continue;
            }
            name_in_scope = n.name();
            if k.is_function() {
                let mut s = String::new();
                self.write_unscoped(c, &mut s);
                s.push('(');
                if let Some(ty) = n.ty()
                    && let Some(p) = self.ast.types.proto(ty)
                {
                    let params = self.ast.types.params(p).to_vec();
                    for (i, &q) in params.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        kiln_ast::types::write_type(
                            &self.ast.types,
                            self,
                            q,
                            kiln_ast::types::PrintOptions::SUGARED,
                            &mut s,
                        );
                    }
                    if p.variadic() {
                        s.push_str(if params.is_empty() { "..." } else { ", ..." });
                    }
                }
                s.push(')');
                chain.push(s);
            } else if self.ast.closure_lambda(c).is_some() {
                chain.push(
                    if self.closure_as_lambda { "(lambda)" } else { "(anonymous class)" }.into(),
                );
            } else if matches!(k, DeclKind::Namespace | DeclKind::CXXRecord | DeclKind::Enum) {
                let mut s = String::new();
                self.write_scope_component(c, &mut s);
                chain.push(s);
            }
            cur = n.ctx();
        }
        for c in chain.iter().rev() {
            out.push_str(c);
            out.push_str("::");
        }
    }

    /// One namespace, class or enumeration of a scope.
    fn write_scope_component(&self, c: DeclId, s: &mut String) {
        if self.ast.decls.get(c).kind() == DeclKind::Namespace {
            s.push_str(&self.simple(c).unwrap_or_else(|| "(anonymous namespace)".into()));
            return;
        }
        self.write_unscoped(c, s);
        if let Some(args) = self.ast.spec_print_args(c) {
            kiln_ast::types::write_arg_list_of(&self.ast.types, self, c, args, s);
        }
    }
}

impl TypeNames for SemaNames<'_> {
    fn clang_compat(&self) -> u32 {
        self.clang_compat
    }

    fn specialization_args(&self, decl: DeclId) -> Option<kiln_ast::ids::ArgListId> {
        self.ast.spec_print_args(decl)
    }

    fn preferred_name(&self, decl: DeclId) -> Option<QualType> {
        self.ast.preferred_name_of(decl)
    }

    fn tag_keyword(&self, decl: DeclId) -> &'static str {
        self.ast.decls.record(decl).map_or("struct", |r| r.tag.keyword())
    }

    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        let k = self.ast.decls.get(decl).kind();
        if scoped
            && (k.is_function() || k == DeclKind::FunctionTemplate)
            && self.in_local_scope(decl)
        {
            self.write_decl_scope(decl, out);
        } else if scoped {
            self.write_scope(decl, out);
        }
        self.write_unscoped(decl, out);
        // A function template specialization prints its arguments
        // (clang `getNameForDiagnostic`): `f<int>`.
        if self.ast.decls.get(decl).kind().is_function()
            && let Some((_, info)) = self.ast.spec_info(decl)
        {
            let typed = SemaNames { typed_args: Some(decl), ..*self };
            kiln_ast::types::write_arg_list_of(&self.ast.types, &typed, decl, info.print_args, out);
        }
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        let n = self.ast.exprs.get(expr);
        match n.kind() {
            // clang `VisitDeclRefExpr`: the qualifier as written, then the
            // name (`H<ns::s::S1>`, WP-186).
            ExprKind::DeclRefExpr => {
                if let Some(q) = self.ast.exprs.decl_ref_extra(expr).and_then(|x| x.qualifier) {
                    kiln_ast::types::write_nns(&self.ast.types, self, q, out);
                }
                out.push_str(&self.simple(self.ast.exprs.decl(expr)).unwrap_or_default());
            }
            ExprKind::ConstantExpr | ExprKind::ImplicitCastExpr => {
                self.write_expr(self.ast.exprs.operand(expr), out);
            }
            // Kept as written (clang `printPretty`): `typeof (i)`.
            ExprKind::ParenExpr => {
                out.push('(');
                self.write_expr(self.ast.exprs.operand(expr), out);
                out.push(')');
            }
            // Everything else (`noexcept(sizeof(int) == 4)` in a
            // function type, ...) prints as clang's `printPretty`.
            _ => TypeNames::write_expr(self.ast, expr, out),
        }
    }

    fn write_value(&self, ty: QualType, value: ConstValueId, out: &mut String) {
        // The constant printer in template-argument style (clang
        // `TemplateArgument::print` / `printIntegral`): enumerators by
        // qualified name (`S<E::a>`), characters as literals (`C<'x'>`),
        // class, pointer and floating values (§6.10).
        kiln_ast::dump::AstNames::new(self.ast).write_value(ty, value, out);
    }

    fn write_value_typed(&self, ty: QualType, value: ConstValueId, out: &mut String) {
        kiln_ast::dump::AstNames::new(self.ast).write_value_typed(ty, value, out);
    }

    fn arg_includes_type(&self, template: DeclId, index: usize) -> bool {
        self.typed_args == Some(template) || self.ast.nttp_has_deduced_type(template, index)
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.ast.symbols().as_str(sym));
    }

    fn write_template_param(&self, associated: DeclId, index: u32, out: &mut String) {
        self.ast.write_replaced_param(associated, index, out);
    }
}
