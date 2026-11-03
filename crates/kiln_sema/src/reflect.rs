//! Reflection (`-freflection`; spec/reflection.md; [expr.reflect],
//! [basic.splice], [meta.reflection]).
//!
//! `^^operand` builds a `CXXReflectExpr` whose constant value is a
//! [`ReflectionId`] interned in `ast.reflections` (see
//! `kiln_ast::reflect` for the entity mapping). A type splice evaluates
//! its operand through the Oven and maps the id back. The `std::meta`
//! queries are consteval intrinsics the Oven dispatches through
//! [`kiln_oven::SemaBridge::meta`]; their sema halves are in `meta/`
//! (spec/reflection.md §3).

use kiln_ast::consts::Scalar;
use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::using::Qualifier;

impl Sema<'_, '_> {
    /// The type `std::meta::info` (spelled `decltype(^^int)`).
    pub fn meta_info_type(&self) -> QualType {
        BuiltinType::MetaInfo.qual_type()
    }

    /// True if `t` is (cv) `std::meta::info`.
    pub fn is_meta_info(&self, t: QualType) -> bool {
        self.ast.types.canonical(t).without_quals() == self.meta_info_type()
    }

    /// `^^type-id` ([expr.reflect]/2).
    pub fn act_on_reflect_type(&mut self, ty: QualType, loc: Loc, end: Loc) -> ExprId {
        let info = self.meta_info_type();
        if ty == QualType::ERROR {
            return self.recover(info, &[], loc);
        }
        self.builder().reflect_expr(Reflectee::Type(ty), info, loc, end)
    }

    /// `^^name` / `^^nns name` ([expr.reflect]/3-5): a namespace, a
    /// variable, a non-overloaded function or a data member. Overload
    /// sets, templates and anything else are not reflected by the spike.
    pub fn act_on_reflect_name(
        &mut self,
        qual: Option<Qualifier>,
        name: Symbol,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let info = self.meta_info_type();
        let lookup = |s: &Self, k| match qual {
            Some(q) => s.lookup_qualified(q.ctx, name, k),
            None => s.lookup_name(name, k),
        };
        let mut found = lookup(self, LookupKind::Ordinary);
        if found.decls.is_empty() {
            found = lookup(self, LookupKind::Nested);
        }
        let text = crate::decl::sym_str(self, name);
        match found.decls.as_slice() {
            [] => {
                let d = self.diag(DiagId::sema_undeclared_var_use, name_loc).arg(text);
                self.emit(d);
                self.recover(info, &[], loc)
            }
            &[d] => match self.kind_of(d) {
                // Spike: an alias reflects the namespace it denotes (P2996
                // reflects the alias itself; `dealias` unwraps it).
                DeclKind::NamespaceAlias => {
                    let d = self.ast.resolve_alias(d);
                    self.builder().reflect_expr(Reflectee::Decl(d), info, loc, end)
                }
                DeclKind::Namespace
                | DeclKind::Var
                | DeclKind::Field
                | DeclKind::Function
                | DeclKind::CXXMethod => {
                    let d = self.ast.first(d);
                    self.builder().reflect_expr(Reflectee::Decl(d), info, loc, end)
                }
                _ => {
                    let d =
                        self.diag(DiagId::sema_reflect_operand, name_loc).arg(format!("'{text}'"));
                    self.emit(d);
                    self.recover(info, &[], loc)
                }
            },
            _ => {
                let d = self
                    .diag(DiagId::sema_reflect_operand, name_loc)
                    .arg(format!("overload set '{text}'"));
                self.emit(d);
                self.recover(info, &[], loc)
            }
        }
    }

    /// The type a splice-type-specifier `[: e :]` designates
    /// ([dcl.type.splice]/2): `e` must be a converted constant expression
    /// of type `std::meta::info` reflecting a type.
    pub fn act_on_splice_type(&mut self, e: ExprId, open: Loc, close: Loc) -> Option<QualType> {
        let _ = close;
        if self.expr_has_error(e) {
            return None;
        }
        let at = self.begin_loc(e);
        if self.expr_is_value_dependent(e) || self.is_dependent_type(self.ast.exprs.get(e).ty()) {
            self.unsupported("dependent splice", open);
            return None;
        }
        let ety = self.ast.exprs.get(e).ty();
        if !self.is_meta_info(ety) {
            let d = self.diag(DiagId::sema_splice_not_info, at).arg(self.ty_arg(ety));
            self.emit(d);
            return None;
        }
        let e = crate::conv::decay(self, e);
        let v = match self.eval_const(e, EvalContext::new(EvalKind::Other, at)) {
            Ok(v) => v,
            Err(f) => {
                let d = self.diag(DiagId::sema_splice_not_constant, at);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                return None;
            }
        };
        let r = match self.ast.consts.scalar(v) {
            Some(Scalar::Reflection { id, .. }) => self.ast.reflections.get(id),
            _ => Reflectee::Null,
        };
        if let Reflectee::Type(t) = r {
            Some(t)
        } else {
            self.error(DiagId::sema_splice_not_type, at);
            None
        }
    }
}
