//! Templates and substitution ([meta.reflection.substitute]; WP-482).
//! `substitute` and `can_substitute` instantiate declarations and can see
//! what is declared at the evaluation point: `PointDependent`.

super::meta_family! {
    /// The template family.
    pub enum TemplOp {
        TemplateOf = ("template_of", "i", Stable, 482);
        TemplateArgumentsOf = ("template_arguments_of", "i", Stable, 482);
        CanSubstitute = ("can_substitute", "ix", PointDependent, 482);
        Substitute = ("substitute", "ix", PointDependent, 482);
    }
}
