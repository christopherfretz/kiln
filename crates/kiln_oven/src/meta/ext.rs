//! kiln extensions and the spike's stand-ins (WP-267, ported by WP-470):
//! declared by `tests/support/kiln_meta.h`. `name_of` and the member ops
//! are retired by WP-475 and WP-478; `__kiln_define_member` stays as a
//! documented extension (spec/reflection.md §6).

super::meta_family! {
    /// The extension family.
    pub enum ExtOp {
        /// `std::meta::name_of(r)`: the name as a `const char *` into a
        /// frozen string literal object.
        NameOf = ("name_of", "i", Stable, 470);
        /// `std::meta::__kiln_members_count(r)`: the number of members of
        /// the class `r` reflects (completing it on demand).
        MembersCount = ("__kiln_members_count", "i", PointDependent, 470);
        /// `std::meta::__kiln_member_at(r, i)`: its `i`th member.
        MemberAt = ("__kiln_member_at", "ix", PointDependent, 470);
        /// `::__kiln_define_member(cls, type, name)`: adds a non-static
        /// data member to the class being defined.
        DefineMember = ("__kiln_define_member", "iix", Mutating, 470);
    }
}
