//! Access contexts ([meta.reflection.access.context],
//! [meta.reflection.access.queries]; P3547; WP-478). The ops that read
//! the evaluation point are `PointDependent`; the access checks take the
//! context as an operand and are `Stable`. `access_context::unprivileged`
//! and `unchecked` are library functions, not intrinsics; `via` checks
//! its operand through the kiln-private `__kiln_access_via`.

super::meta_family! {
    /// The access family.
    pub enum AccessOp {
        /// `access_context::current()` (a static member function).
        Current = ("access_context::current", "", PointDependent, 478);
        CurrentFunction = ("current_function", "", PointDependent, 478);
        CurrentClass = ("current_class", "", PointDependent, 478);
        CurrentNamespace = ("current_namespace", "", PointDependent, 478);
        IsAccessible = ("is_accessible", "ix", Stable, 478);
        HasInaccessibleNonstaticDataMembers =
            ("has_inaccessible_nonstatic_data_members", "ix", Stable, 478);
        HasInaccessibleBases = ("has_inaccessible_bases", "ix", Stable, 478);
        HasInaccessibleSubobjects = ("has_inaccessible_subobjects", "ix", Stable, 478);
        /// `std::meta::__kiln_access_via(cls)`: `cls` if it is null or
        /// reflects a complete class type, else a failure
        /// (`access_context::via`'s precondition).
        KilnVia = ("__kiln_access_via", "i", Stable, 478);
    }
}
