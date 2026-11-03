//! Type traits ([meta.reflection.traits]): every op delegates to sema's
//! builtin trait engine (WP-477). All are `Stable`: a trait of an
//! incomplete type is a failure (never cached), and a successful answer
//! cannot change.

super::meta_family! {
    /// The type-trait family.
    pub enum TraitOp {
        IsVoidType = ("is_void_type", "i", Stable, 477);
        IsNullPointerType = ("is_null_pointer_type", "i", Stable, 477);
        IsIntegralType = ("is_integral_type", "i", Stable, 477);
        IsFloatingPointType = ("is_floating_point_type", "i", Stable, 477);
        IsArrayType = ("is_array_type", "i", Stable, 477);
        IsPointerType = ("is_pointer_type", "i", Stable, 477);
        IsLvalueReferenceType = ("is_lvalue_reference_type", "i", Stable, 477);
        IsRvalueReferenceType = ("is_rvalue_reference_type", "i", Stable, 477);
        IsMemberObjectPointerType = ("is_member_object_pointer_type", "i", Stable, 477);
        IsMemberFunctionPointerType = ("is_member_function_pointer_type", "i", Stable, 477);
        IsEnumType = ("is_enum_type", "i", Stable, 477);
        IsUnionType = ("is_union_type", "i", Stable, 477);
        IsClassType = ("is_class_type", "i", Stable, 477);
        IsFunctionType = ("is_function_type", "i", Stable, 477);
        IsReflectionType = ("is_reflection_type", "i", Stable, 477);
        IsReferenceType = ("is_reference_type", "i", Stable, 477);
        IsArithmeticType = ("is_arithmetic_type", "i", Stable, 477);
        IsFundamentalType = ("is_fundamental_type", "i", Stable, 477);
        IsObjectType = ("is_object_type", "i", Stable, 477);
        IsScalarType = ("is_scalar_type", "i", Stable, 477);
        IsCompoundType = ("is_compound_type", "i", Stable, 477);
        IsMemberPointerType = ("is_member_pointer_type", "i", Stable, 477);
        IsConstType = ("is_const_type", "i", Stable, 477);
        IsVolatileType = ("is_volatile_type", "i", Stable, 477);
        IsTriviallyCopyableType = ("is_trivially_copyable_type", "i", Stable, 477);
        IsStandardLayoutType = ("is_standard_layout_type", "i", Stable, 477);
        IsEmptyType = ("is_empty_type", "i", Stable, 477);
        IsPolymorphicType = ("is_polymorphic_type", "i", Stable, 477);
        IsAbstractType = ("is_abstract_type", "i", Stable, 477);
        IsFinalType = ("is_final_type", "i", Stable, 477);
        IsAggregateType = ("is_aggregate_type", "i", Stable, 477);
        IsStructuralType = ("is_structural_type", "i", Stable, 477);
        IsSignedType = ("is_signed_type", "i", Stable, 477);
        IsUnsignedType = ("is_unsigned_type", "i", Stable, 477);
        IsBoundedArrayType = ("is_bounded_array_type", "i", Stable, 477);
        IsUnboundedArrayType = ("is_unbounded_array_type", "i", Stable, 477);
        IsScopedEnumType = ("is_scoped_enum_type", "i", Stable, 477);
        IsConstructibleType = ("is_constructible_type", "ix", Stable, 477);
        IsDefaultConstructibleType = ("is_default_constructible_type", "i", Stable, 477);
        IsCopyConstructibleType = ("is_copy_constructible_type", "i", Stable, 477);
        IsMoveConstructibleType = ("is_move_constructible_type", "i", Stable, 477);
        IsAssignableType = ("is_assignable_type", "ii", Stable, 477);
        IsCopyAssignableType = ("is_copy_assignable_type", "i", Stable, 477);
        IsMoveAssignableType = ("is_move_assignable_type", "i", Stable, 477);
        IsSwappableWithType = ("is_swappable_with_type", "ii", Stable, 477);
        IsSwappableType = ("is_swappable_type", "i", Stable, 477);
        IsDestructibleType = ("is_destructible_type", "i", Stable, 477);
        IsTriviallyConstructibleType = ("is_trivially_constructible_type", "ix", Stable, 477);
        IsTriviallyDefaultConstructibleType =
            ("is_trivially_default_constructible_type", "i", Stable, 477);
        IsTriviallyCopyConstructibleType =
            ("is_trivially_copy_constructible_type", "i", Stable, 477);
        IsTriviallyMoveConstructibleType =
            ("is_trivially_move_constructible_type", "i", Stable, 477);
        IsTriviallyAssignableType = ("is_trivially_assignable_type", "ii", Stable, 477);
        IsTriviallyCopyAssignableType = ("is_trivially_copy_assignable_type", "i", Stable, 477);
        IsTriviallyMoveAssignableType = ("is_trivially_move_assignable_type", "i", Stable, 477);
        IsTriviallyDestructibleType = ("is_trivially_destructible_type", "i", Stable, 477);
        IsNothrowConstructibleType = ("is_nothrow_constructible_type", "ix", Stable, 477);
        IsNothrowDefaultConstructibleType =
            ("is_nothrow_default_constructible_type", "i", Stable, 477);
        IsNothrowCopyConstructibleType = ("is_nothrow_copy_constructible_type", "i", Stable, 477);
        IsNothrowMoveConstructibleType = ("is_nothrow_move_constructible_type", "i", Stable, 477);
        IsNothrowAssignableType = ("is_nothrow_assignable_type", "ii", Stable, 477);
        IsNothrowCopyAssignableType = ("is_nothrow_copy_assignable_type", "i", Stable, 477);
        IsNothrowMoveAssignableType = ("is_nothrow_move_assignable_type", "i", Stable, 477);
        IsNothrowSwappableWithType = ("is_nothrow_swappable_with_type", "ii", Stable, 477);
        IsNothrowSwappableType = ("is_nothrow_swappable_type", "i", Stable, 477);
        IsNothrowDestructibleType = ("is_nothrow_destructible_type", "i", Stable, 477);
        IsImplicitLifetimeType = ("is_implicit_lifetime_type", "i", Stable, 477);
        HasVirtualDestructor = ("has_virtual_destructor", "i", Stable, 477);
        HasUniqueObjectRepresentations =
            ("has_unique_object_representations", "i", Stable, 477);
        ReferenceConstructsFromTemporary =
            ("reference_constructs_from_temporary", "ii", Stable, 477);
        ReferenceConvertsFromTemporary = ("reference_converts_from_temporary", "ii", Stable, 477);
        Rank = ("rank", "i", Stable, 477);
        Extent = ("extent", "ix", Stable, 477);
        IsSameType = ("is_same_type", "ii", Stable, 477);
        IsBaseOfType = ("is_base_of_type", "ii", Stable, 477);
        IsVirtualBaseOfType = ("is_virtual_base_of_type", "ii", Stable, 477);
        IsConvertibleType = ("is_convertible_type", "ii", Stable, 477);
        IsNothrowConvertibleType = ("is_nothrow_convertible_type", "ii", Stable, 477);
        IsLayoutCompatibleType = ("is_layout_compatible_type", "ii", Stable, 477);
        IsPointerInterconvertibleBaseOfType =
            ("is_pointer_interconvertible_base_of_type", "ii", Stable, 477);
        IsInvocableType = ("is_invocable_type", "ix", Stable, 477);
        IsInvocableRType = ("is_invocable_r_type", "iix", Stable, 477);
        IsNothrowInvocableType = ("is_nothrow_invocable_type", "ix", Stable, 477);
        IsNothrowInvocableRType = ("is_nothrow_invocable_r_type", "iix", Stable, 477);
        RemoveConst = ("remove_const", "i", Stable, 477);
        RemoveVolatile = ("remove_volatile", "i", Stable, 477);
        RemoveCv = ("remove_cv", "i", Stable, 477);
        AddConst = ("add_const", "i", Stable, 477);
        AddVolatile = ("add_volatile", "i", Stable, 477);
        AddCv = ("add_cv", "i", Stable, 477);
        RemoveReference = ("remove_reference", "i", Stable, 477);
        AddLvalueReference = ("add_lvalue_reference", "i", Stable, 477);
        AddRvalueReference = ("add_rvalue_reference", "i", Stable, 477);
        MakeSigned = ("make_signed", "i", Stable, 477);
        MakeUnsigned = ("make_unsigned", "i", Stable, 477);
        RemoveExtent = ("remove_extent", "i", Stable, 477);
        RemoveAllExtents = ("remove_all_extents", "i", Stable, 477);
        RemovePointer = ("remove_pointer", "i", Stable, 477);
        AddPointer = ("add_pointer", "i", Stable, 477);
        RemoveCvref = ("remove_cvref", "i", Stable, 477);
        Decay = ("decay", "i", Stable, 477);
        CommonType = ("common_type", "x", Stable, 477);
        CommonReference = ("common_reference", "x", Stable, 477);
        UnderlyingType = ("underlying_type", "i", Stable, 477);
        InvokeResult = ("invoke_result", "ix", Stable, 477);
        UnwrapReference = ("unwrap_reference", "i", Stable, 477);
        UnwrapRefDecay = ("unwrap_ref_decay", "i", Stable, 477);
        TupleSize = ("tuple_size", "i", Stable, 477);
        TupleElement = ("tuple_element", "xi", Stable, 477);
        VariantSize = ("variant_size", "i", Stable, 477);
        VariantAlternative = ("variant_alternative", "xi", Stable, 477);
        TypeOrder = ("type_order", "ii", Stable, 477);
        // The range forms as `(const info* data, size_t n)` overloads
        // (spec/reflection-traits.md §1.3): `kiln_meta_traits.h` forwards
        // each IS `reflection_range` template to one of these until span
        // reading lands (WP-475).
        IsConstructibleTypeN = ("is_constructible_type", "ixx", Stable, 477);
        IsTriviallyConstructibleTypeN = ("is_trivially_constructible_type", "ixx", Stable, 477);
        IsNothrowConstructibleTypeN = ("is_nothrow_constructible_type", "ixx", Stable, 477);
        IsInvocableTypeN = ("is_invocable_type", "ixx", Stable, 477);
        IsInvocableRTypeN = ("is_invocable_r_type", "iixx", Stable, 477);
        IsNothrowInvocableTypeN = ("is_nothrow_invocable_type", "ixx", Stable, 477);
        IsNothrowInvocableRTypeN = ("is_nothrow_invocable_r_type", "iixx", Stable, 477);
        InvokeResultN = ("invoke_result", "ixx", Stable, 477);
        CommonTypeN = ("common_type", "xx", Stable, 477);
        CommonReferenceN = ("common_reference", "xx", Stable, 477);
    }
}
