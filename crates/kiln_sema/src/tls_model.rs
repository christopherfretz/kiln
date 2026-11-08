//! GNU `tls_model("model")` (spec/sema.md §4.5; clang `handleTLSModelAttr`,
//! `AttrDocs` `tls_model`): the TLS model of a thread-local variable,
//! one of `global-dynamic`, `local-dynamic`, `initial-exec` and
//! `local-exec`, which codegen gives its global (clang `setTLSMode`).
//!
//! [`Sema::tls_model_attribute`] checks the first argument is a string
//! literal (clang's parser does) and keeps it with the argument count;
//! [`Sema::check_tls_model_attrs`] checks the rest in clang's order (the
//! subject, an error, the count, then the model) and replaces the node
//! with one whose `a` is the model.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclKind, TlsKind};
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

/// The models, in the order of the checked node's `a` (from 1), which
/// is `kiln_llvm::TlsModel`'s.
const MODELS: [&str; 4] = ["global-dynamic", "local-dynamic", "initial-exec", "local-exec"];

impl Sema<'_, '_> {
    /// `tls_model` as written: `a` = the raw `ExprId` of the first
    /// argument (0: none), `b` = the argument count; `None` after
    /// diagnosing a first argument that is not a string literal.
    pub(crate) fn tls_model_attribute(
        &mut self,
        syntax: AttrSyntax,
        shown: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let a = match args.first() {
            Some(AttrArg::Expr(e)) if self.is_err_expr(*e) => return None,
            Some(AttrArg::Expr(e)) if self.string_lit_of(*e).is_some() => e.raw(),
            None => 0,
            Some(arg) => {
                let at = match arg {
                    AttrArg::Expr(e) => self.begin_loc(*e),
                    AttrArg::Ident(_, at) => *at,
                    AttrArg::Type(_) => loc,
                };
                let msg =
                    self.diag(DiagId::parse_attribute_requires_string, at).arg(shown.to_owned());
                self.emit(msg);
                return None;
            }
        };
        let b = u32::try_from(args.len()).unwrap_or(u32::MAX);
        Some(self.ast.attrs.push(AttrNode {
            kind: AttrKind::TLS_MODEL,
            syntax,
            flags: 0,
            loc,
            a,
            b,
        }))
    }

    /// Checks the `tls_model` attributes among `attrs` of `d`, dropping
    /// the invalid ones and replacing each valid one by a node whose `a`
    /// is the model (1-based in [`MODELS`]).
    pub(crate) fn check_tls_model_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if node.kind != AttrKind::TLS_MODEL {
                i += 1;
                continue;
            }
            match self.tls_model_of(d, node) {
                Some(m) => {
                    attrs[i] = self.ast.attrs.push(AttrNode { a: m, b: 0, ..node });
                    i += 1;
                }
                None => {
                    attrs.remove(i);
                }
            }
        }
    }

    /// The model `tls_model` attribute `node` on `d` names, or `None`
    /// after a diagnostic.
    fn tls_model_of(&mut self, d: DeclId, node: AttrNode) -> Option<u32> {
        let shown = self.attr_shown(&node, "tls_model");
        // `SubjectList<[TLSVar], ErrorDiag>`.
        let tls = matches!(self.kind_of(d), DeclKind::Var | DeclKind::VarTemplateSpecialization)
            && self.ast.decls.var(d).is_some_and(|v| v.spec.tls() != TlsKind::None);
        if !tls {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                .arg(format!("'{shown}'"))
                .arg("thread-local variables");
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
        self.error(DiagId::sema_attr_tlsmodel_arg, self.begin_loc(e));
        None
    }
}
