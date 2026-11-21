//! GNU `model("small" | "large")` (spec/sema.md §4.5; clang 22
//! `handleCodeModelAttr`, x86-64 only; WP-1745): the code model of a
//! non-TLS global variable, which codegen gives its global
//! (`code_model "large"`, clang `GetOrCreateLLVMGlobal`). clang 19 and
//! clang 22 on `AArch64` do not know the attribute (the triage answers
//! `Unknown` there); every release's parser still requires a string
//! literal first argument (`kiln_parse::attr`).
//!
//! [`Sema::code_model_attribute`] keeps the first argument and the
//! count; [`Sema::check_code_model_attrs`] checks the rest in clang's
//! order (the subject, the count, then the model) and replaces the node
//! with one whose `a` is the model.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclKind, TlsKind};
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

/// The models x86-64 accepts (clang `isValidCodeModelAttr`), in the
/// order of the checked node's `a` (from 1).
const MODELS: [&str; 2] = ["small", "large"];

impl Sema<'_, '_> {
    /// `model` as written: `a` = the raw `ExprId` of the first argument
    /// (0: none; the parser has checked it is a string literal), `b` =
    /// the argument count.
    pub(crate) fn code_model_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let a = match args.first() {
            Some(AttrArg::Expr(e)) if self.is_err_expr(*e) => return None,
            Some(AttrArg::Expr(e)) => e.raw(),
            _ => 0,
        };
        let b = u32::try_from(args.len()).unwrap_or(u32::MAX);
        Some(self.ast.attrs.push(AttrNode {
            kind: AttrKind::CODE_MODEL,
            syntax,
            flags: 0,
            loc,
            a,
            b,
        }))
    }

    /// Checks the `model` attributes among `attrs` of `d`, dropping the
    /// invalid ones and replacing each valid one by a node whose `a` is
    /// the model (1-based in [`MODELS`]).
    pub(crate) fn check_code_model_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if node.kind != AttrKind::CODE_MODEL || node.b == 0 && node.a != 0 {
                i += 1;
                continue;
            }
            match self.code_model_of(d, node) {
                Some(m) => {
                    // `b` 0 with `a` set marks a checked node.
                    attrs[i] = self.ast.attrs.push(AttrNode { a: m, b: 0, ..node });
                    i += 1;
                }
                None => {
                    attrs.remove(i);
                }
            }
        }
    }

    /// The model `model` attribute `node` on `d` names, or `None` after
    /// a diagnostic.
    fn code_model_of(&mut self, d: DeclId, node: AttrNode) -> Option<u32> {
        let shown = self.attr_shown(&node, "model");
        // `SubjectList<[NonTLSGlobalVar], ErrorDiag>`.
        let global = matches!(self.kind_of(d), DeclKind::Var | DeclKind::VarTemplateSpecialization)
            && self.has_global_storage(d)
            && self.ast.decls.var(d).is_some_and(|v| v.spec.tls() == TlsKind::None);
        if !global {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                .arg(format!("'{shown}'"))
                .arg("non-TLS global variables");
            self.emit(msg);
            return None;
        }
        if node.b != 1 {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, node.loc).arg(shown).arg("1");
            self.emit(msg);
            return None;
        }
        let e = ExprId::from_raw(node.a)?;
        let lit = self.string_lit_of(e)?;
        let id = self.ast.exprs.string_lit(lit);
        let model = self.ast.strings.bytes(id);
        if let Some(m) = MODELS.iter().position(|m| m.as_bytes() == model) {
            return u32::try_from(m + 1).ok();
        }
        let shown_model = String::from_utf8_lossy(model).into_owned();
        let msg =
            self.diag(DiagId::sema_attr_unsupported_code_model, self.begin_loc(e)).arg(shown_model);
        self.emit(msg);
        None
    }
}
