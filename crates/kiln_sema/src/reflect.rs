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

use kiln_ast::reflect::Reflectee;
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
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
        let r = match self.splice_entity(e) {
            crate::splice::Spliced::Error => return None,
            crate::splice::Spliced::Dependent => {
                let p = self.dependent_profile(e);
                return Some(self.ast.types.splice_type(e, Some(p), None));
            }
            crate::splice::Spliced::Entity(r) => r,
        };
        let at = self.begin_loc(e);
        let _ = open;
        if let Reflectee::Type(t) = r {
            Some(t)
        } else {
            self.error(DiagId::sema_splice_not_type, at);
            None
        }
    }
}
