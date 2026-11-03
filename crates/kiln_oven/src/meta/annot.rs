//! Annotation queries ([meta.reflection.annotation]; P3394; WP-478, the
//! syntax is WP-481). A later redeclaration can add annotations, so the
//! enumerations are `PointDependent`.

super::meta_family! {
    /// The annotation family.
    pub enum AnnotOp {
        AnnotationsOf = ("annotations_of", "i", PointDependent, 478);
        AnnotationsOfWithType = ("annotations_of_with_type", "ii", PointDependent, 478);
        IsAnnotation = ("is_annotation", "i", Stable, 478);
    }
}
