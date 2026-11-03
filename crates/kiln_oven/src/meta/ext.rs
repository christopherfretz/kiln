//! kiln extensions and the spike's stand-ins (WP-267, ported by WP-470):
//! declared by `tests/support/kiln_meta.h`. `name_of` gives way to
//! WP-475's `identifier_of` (the spike's member ops were retired by
//! WP-478 for `members_of`); `__kiln_define_member` stays as a documented extension
//! (spec/reflection.md §6).

super::meta_family! {
    /// The extension family.
    pub enum ExtOp {
        /// `std::meta::name_of(r)`: the name as a `const char *` into a
        /// frozen string literal object.
        NameOf = ("name_of", "i", Stable, 470);
        /// `::__kiln_define_member(cls, type, name)`: adds a non-static
        /// data member to the class being defined.
        DefineMember = ("__kiln_define_member", "iix", Mutating, 470);
    }
}
