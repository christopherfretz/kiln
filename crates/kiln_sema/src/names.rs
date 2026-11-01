//! [`TypeNames`] over a [`Sema`]'s AST: how declarations, expressions and
//! values print inside types in diagnostics.
//!
//! A subset of the parser package's (WP-048) printer of the same name;
//! on merge, that one is kept.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprFlags, ExprKind};
use kiln_ast::types::TypeNames;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType};
use kiln_source::symbol::Symbol;

use crate::Sema;

/// Prints names from an AST.
#[derive(Debug)]
pub struct SemaNames<'s> {
    ast: &'s Ast,
    /// Where unnamed tags' locations resolve (`(lambda at F:L:C)`).
    sources: Option<&'s kiln_source::SourceManager>,
}

impl<'s> SemaNames<'s> {
    /// Names for `sema`'s AST.
    pub fn new(sema: &'s Sema<'_, '_>) -> Self {
        SemaNames { ast: &sema.ast, sources: Some(sema.diags.sm()) }
    }

    /// Names for `ast` alone.
    pub fn of_ast(ast: &'s Ast) -> Self {
        SemaNames { ast, sources: None }
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
        while let Some(c) = cur {
            let n = self.ast.decls.get(c);
            match n.kind() {
                DeclKind::Namespace if self.redundant_inline(c, name_in_scope) => {}
                DeclKind::Namespace => {
                    chain.push(self.simple(c).unwrap_or_else(|| "(anonymous namespace)".into()));
                    name_in_scope = n.name();
                }
                DeclKind::CXXRecord | DeclKind::Enum => {
                    let mut s = String::new();
                    self.write_unscoped(c, &mut s);
                    if let Some(args) = self.ast.spec_print_args(c) {
                        kiln_ast::types::write_arg_list(&self.ast.types, self, args, &mut s);
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
    /// function as `f(int)`, a closure type as `(anonymous class)`.
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
                chain.push("(anonymous class)".into());
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
            kiln_ast::types::write_arg_list(&self.ast.types, self, args, s);
        }
    }
}

impl TypeNames for SemaNames<'_> {
    fn specialization_args(&self, decl: DeclId) -> Option<kiln_ast::ids::ArgListId> {
        self.ast.spec_print_args(decl)
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
            kiln_ast::types::write_arg_list(&self.ast.types, self, info.print_args, out);
        }
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        let n = self.ast.exprs.get(expr);
        match n.kind() {
            ExprKind::IntegerLiteral if !n.has(ExprFlags::WIDE) => {
                let v = u64::from(n.a()) | (u64::from(n.b()) << 32);
                out.push_str(&v.to_string());
            }
            ExprKind::DeclRefExpr => {
                out.push_str(&self.simple(self.ast.exprs.decl(expr)).unwrap_or_default());
            }
            ExprKind::ConstantExpr | ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => {
                self.write_expr(self.ast.exprs.operand(expr), out);
            }
            // Everything else (`noexcept(sizeof(int) == 4)` in a
            // function type, ...) prints as clang's `printPretty`.
            _ => TypeNames::write_expr(self.ast, expr, out),
        }
    }

    fn write_value(&self, ty: QualType, value: ConstValueId, out: &mut String) {
        match self.ast.consts.scalar(value) {
            Some(Scalar::Int { signed: true, bits, .. }) => match bits.to_i128() {
                Some(v) => out.push_str(&v.to_string()),
                None => out.push_str("<value>"),
            },
            Some(Scalar::Int { bits, .. }) => match bits.to_u128() {
                Some(v) => out.push_str(&v.to_string()),
                None => out.push_str("<value>"),
            },
            Some(Scalar::Bool { value, .. }) => out.push_str(if value { "true" } else { "false" }),
            // Class, pointer and floating values (§6.10): the constant
            // printer.
            _ => kiln_ast::dump::AstNames::new(self.ast).write_value(ty, value, out),
        }
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.ast.symbols().as_str(sym));
    }

    fn write_template_param(&self, associated: DeclId, index: u32, out: &mut String) {
        self.ast.write_replaced_param(associated, index, out);
    }
}
