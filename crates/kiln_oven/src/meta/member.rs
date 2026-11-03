//! Member enumeration and layout ([meta.reflection.member.queries],
//! [meta.reflection.layout]; WP-478). The class queries are `Stable`: a
//! class's members are fixed once it is complete, and an incomplete class
//! is a failure (never cached). `members_of` is `PointDependent` because a
//! namespace grows.

super::meta_family! {
    /// The member family.
    pub enum MemberOp {
        MembersOf = ("members_of", "ix", PointDependent, 478);
        BasesOf = ("bases_of", "ix", Stable, 478);
        StaticDataMembersOf = ("static_data_members_of", "ix", Stable, 478);
        NonstaticDataMembersOf = ("nonstatic_data_members_of", "ix", Stable, 478);
        SubobjectsOf = ("subobjects_of", "ix", Stable, 478);
        EnumeratorsOf = ("enumerators_of", "i", Stable, 478);
        ParametersOf = ("parameters_of", "i", Stable, 478);
        OffsetOf = ("offset_of", "i", Stable, 478);
        SizeOf = ("size_of", "i", Stable, 478);
        AlignmentOf = ("alignment_of", "i", Stable, 478);
        BitSizeOf = ("bit_size_of", "i", Stable, 478);
    }
}
