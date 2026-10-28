//! [`TypeNames`] over a [`Sema`]'s AST: how declarations, expressions and
//! values print inside types in diagnostics (clang's `PrintingPolicy` as
//! used by `Sema::Diag`).

use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::TypeNames;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType};
use kiln_source::SourceManager;
use kiln_source::symbol::Symbol;

use crate::Sema;

/// Prints names from an AST (and locations of unnamed tags from the
/// source manager).
#[derive(Debug)]
pub struct SemaNames<'s> {
    ast: &'s Ast,
    sm: Option<&'s SourceManager>,
}

impl<'s> SemaNames<'s> {
    /// Names for `sema`'s AST.
    pub fn new(sema: &'s Sema<'_, '_>) -> Self {
        SemaNames { ast: &sema.ast, sm: Some(sema.diags.sm()) }
    }

    /// Names for `ast` alone (unnamed tags print without a location).
    pub fn of_ast(ast: &'s Ast) -> Self {
        SemaNames { ast, sm: None }
    }

    fn simple(&self, d: DeclId) -> Option<String> {
        let name = self.ast.decls.get(d).name();
        if name.is_empty() {
            return None;
        }
        self.ast.names.simple_spelling(name, self.ast.symbols())
    }

    fn write_scope(&self, d: DeclId, out: &mut String) {
        let mut chain = Vec::new();
        let mut cur = self.ast.decls.get(d).ctx();
        while let Some(c) = cur {
            let n = self.ast.decls.get(c);
            match n.kind() {
                DeclKind::Namespace => {
                    let inline = self
                        .ast
                        .decls
                        .namespace(self.ast.first(c))
                        .is_some_and(kiln_ast::decl::NamespaceData::is_inline);
                    if !inline {
                        chain.push(self.simple(c).unwrap_or_else(|| "(anonymous namespace)".into()));
                    }
                }
                DeclKind::CXXRecord | DeclKind::Enum => {
                    let mut s = String::new();
                    self.write_unscoped(c, &mut s);
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

    fn write_unscoped(&self, d: DeclId, out: &mut String) {
        if let Some(s) = self.simple(d) {
            out.push_str(&s);
            return;
        }
        let n = self.ast.decls.get(d);
        let what = match n.kind() {
            DeclKind::Enum => "enum",
            _ => match self.ast.decls.record(d).map(|r| r.tag) {
                Some(TagKind::Union) => "union",
                Some(TagKind::Class) => "class",
                _ => "struct",
            },
        };
        out.push_str("(unnamed ");
        out.push_str(what);
        if let Some(lc) = self.sm.and_then(|sm| sm.presumed_line_col(n.loc())) {
            out.push_str(&format!(" at {}:{}:{}", lc.file_name, lc.line, lc.col));
        }
        out.push(')');
    }
}

impl TypeNames for SemaNames<'_> {
    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        if scoped {
            self.write_scope(decl, out);
        }
        self.write_unscoped(decl, out);
    }

    fn write_expr(&self, expr: ExprId, out: &mut String) {
        let n = self.ast.exprs.get(expr);
        match n.kind() {
            ExprKind::IntegerLiteral if !n.has(kiln_ast::expr::ExprFlags::WIDE) => {
                let v = u64::from(n.a()) | (u64::from(n.b()) << 32);
                out.push_str(&v.to_string());
            }
            ExprKind::DeclRefExpr => {
                let d = DeclId::new(n.a());
                out.push_str(&self.simple(d).unwrap_or_default());
            }
            ExprKind::ConstantExpr | ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => {
                self.write_expr(ExprId::new(n.a()), out);
            }
            _ => out.push_str("<expr>"),
        }
    }

    fn write_value(&self, _ty: QualType, value: ConstValueId, out: &mut String) {
        match self.ast.consts.scalar(value) {
            Some(kiln_ast::consts::Scalar::Int { signed, bits, .. }) => {
                bits.write_decimal(signed, out);
            }
            Some(kiln_ast::consts::Scalar::Bool { value, .. }) => {
                out.push_str(if value { "true" } else { "false" });
            }
            _ => out.push_str("<value>"),
        }
    }

    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(self.ast.symbols().as_str(sym));
    }
}
