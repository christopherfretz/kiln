//! The reflection table's limit (spec/sema.md "Reflection (spike,
//! -freflection)"; WP-471): interning one reflection past
//! [`kiln_ast::reflect::ReflectionTable`]'s capacity is a fatal
//! `sema_too_many_reflections`, never a panic.

use kiln_ast::reflect::Reflectee;
use kiln_ast::{ExprId, QualType, ReflectionId};
use kiln_diag::DiagId;
use kiln_oven::Note;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// The id of `r`, interning it. When the table is full: a fatal
    /// `sema_too_many_reflections` at `loc`, and an evaluation failure
    /// with no further notes.
    pub(crate) fn intern_reflection(
        &mut self,
        r: Reflectee,
        loc: Loc,
    ) -> Result<ReflectionId, Vec<Note>> {
        self.ast.reflections.intern(r).map_err(|full| {
            let d = self.diag(DiagId::sema_too_many_reflections, loc).arg(full.limit);
            self.emit(d);
            Vec::new()
        })
    }

    /// `^^operand` of type `ty` ([expr.reflect]); when the table is full,
    /// the fatal `sema_too_many_reflections` and a recovery expression.
    pub(crate) fn build_reflect_expr(
        &mut self,
        operand: Reflectee,
        ty: QualType,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let e = self.builder().reflect_expr(operand, ty, loc, end);
        if self.ast.exprs.get(e).is_value_dependent() || self.ast.exprs.reflect_value(e).is_some() {
            return e;
        }
        // The builder's interning failed: the table is full. Report it
        // through the one helper, with the builder's key (a type's
        // reflection type).
        let key = match operand {
            Reflectee::Type(t) => Reflectee::Type(self.ast.reflection_type(t)),
            r => r,
        };
        match self.intern_reflection(key, loc) {
            Ok(_) => self.builder().reflect_expr(operand, ty, loc, end),
            Err(_) => self.recover(ty, &[], loc),
        }
    }
}
