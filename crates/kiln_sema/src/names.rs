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
}

impl<'s> SemaNames<'s> {
    /// Names for `sema`'s AST.
    pub fn new(sema: &'s Sema<'_, '_>) -> Self {
        SemaNames { ast: &sema.ast }
    }

    /// Names for `ast` alone.
    pub fn of_ast(ast: &'s Ast) -> Self {
        SemaNames { ast }
    }

    fn simple(&self, d: DeclId) -> Option<String> {
        let name = self.ast.decls.get(d).name();
        let s = self.ast.names.simple_spelling(name, self.ast.symbols()).unwrap_or_else(|| {
            // Constructor, destructor and conversion-function names
            // (`X`, `~X`, `operator int`).
            let mut s = String::new();
            kiln_ast::dump::AstNames::new(self.ast).write_decl_name_value(name, &mut s);
            s
        });
        (!s.is_empty()).then_some(s)
    }

    fn write_unscoped(&self, d: DeclId, out: &mut String) {
        if let Some(s) = self.simple(d) {
            out.push_str(&s);
        } else if self.ast.decls.get(d).kind() == DeclKind::Enum {
            out.push_str("(unnamed enum)");
        } else {
            out.push_str("(unnamed struct)");
        }
    }

    fn write_scope(&self, d: DeclId, out: &mut String) {
        let mut chain = Vec::new();
        let mut cur = self.ast.decls.get(d).ctx();
        while let Some(c) = cur {
            let n = self.ast.decls.get(c);
            match n.kind() {
                DeclKind::Namespace => {
                    chain.push(self.simple(c).unwrap_or_else(|| "(anonymous namespace)".into()));
                }
                DeclKind::CXXRecord | DeclKind::Enum => {
                    let mut s = String::new();
                    self.write_unscoped(c, &mut s);
                    if let Some(args) = self.ast.spec_print_args(c) {
                        kiln_ast::types::write_arg_list(&self.ast.types, self, args, &mut s);
                    }
                    chain.push(s);
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

impl TypeNames for SemaNames<'_> {
    fn specialization_args(&self, decl: DeclId) -> Option<kiln_ast::ids::ArgListId> {
        self.ast.spec_print_args(decl)
    }

    fn tag_keyword(&self, decl: DeclId) -> &'static str {
        self.ast.decls.record(decl).map_or("struct", |r| r.tag.keyword())
    }

    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        if scoped {
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
}
