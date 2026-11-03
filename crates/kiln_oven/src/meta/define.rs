//! `data_member_spec` and `define_aggregate`
//! ([meta.reflection.define.aggregate]; WP-480). A spec is interned by
//! content (`Stable`); `define_aggregate` completes a class (`Mutating`).

super::meta_family! {
    /// The definition family.
    pub enum DefineOp {
        DataMemberSpec = ("data_member_spec", "ix", Stable, 480);
        IsDataMemberSpec = ("is_data_member_spec", "i", Stable, 480);
        DefineAggregate = ("define_aggregate", "ix", Mutating, 480);
    }
}
