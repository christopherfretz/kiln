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
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// The type `std::meta::info` (spelled `decltype(^^int)`).
    pub fn meta_info_type(&self) -> QualType {
        BuiltinType::MetaInfo.qual_type()
    }

    /// True if `t` is (cv) `std::meta::info`.
    pub fn is_meta_info(&self, t: QualType) -> bool {
        self.ast.types.canonical(t).without_quals() == self.meta_info_type()
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
