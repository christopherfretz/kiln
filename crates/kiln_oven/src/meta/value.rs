//! Values and static storage ([meta.reflection.extract],
//! [meta.reflection.result], [meta.define.static]; P3491; WP-479). The
//! results are interned by content (spec/reflection.md §2), so each op is
//! a pure function of its operand: `Stable`. `define_static_string`,
//! `define_static_array` and `define_static_object` are library functions
//! over these, not intrinsics.

super::meta_family! {
    /// The value family.
    pub enum ValueOp {
        /// `extract<T>(r)` (a specialization's signature).
        Extract = ("extract", "i", Stable, 479);
        ReflectConstant = ("reflect_constant", "x", Stable, 479);
        ReflectObject = ("reflect_object", "x", Stable, 479);
        ReflectFunction = ("reflect_function", "x", Stable, 479);
        ReflectConstantString = ("reflect_constant_string", "x", Stable, 479);
        ReflectConstantArray = ("reflect_constant_array", "x", Stable, 479);
    }
}
