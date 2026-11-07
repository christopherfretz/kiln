//! Entity queries, names and predicates ([meta.reflection.queries],
//! [meta.reflection.names]; P3096 parameters). spec/reflection.md §3.3
//! has the purity rationale per row.

super::meta_family! {
    /// The query family.
    pub enum QueryOp {
        /// `type_of(r)`: the type of the reflected entity (a variable's as
        /// its latest declaration gives it; one still an array of unknown
        /// bound makes the call impure, spec/reflection-queries.md §1).
        TypeOf = ("type_of", "i", Stable, 470);
        ObjectOf = ("object_of", "i", Stable, 476);
        ConstantOf = ("constant_of", "i", Stable, 476);
        ParentOf = ("parent_of", "i", Stable, 476);
        Dealias = ("dealias", "i", Stable, 476);
        HasTemplateArguments = ("has_template_arguments", "i", Stable, 476);
        OperatorOf = ("operator_of", "i", Stable, 476);
        SymbolOf = ("symbol_of", "x", Stable, 476);
        U8SymbolOf = ("u8symbol_of", "x", Stable, 476);
        HasIdentifier = ("has_identifier", "i", Stable, 475);
        IdentifierOf = ("identifier_of", "i", Stable, 475);
        U8IdentifierOf = ("u8identifier_of", "i", Stable, 475);
        DisplayStringOf = ("display_string_of", "i", Stable, 475);
        U8DisplayStringOf = ("u8display_string_of", "i", Stable, 475);
        SourceLocationOf = ("source_location_of", "i", Stable, 475);
        /// Valid only inside the definition of the parameter's function: the
        /// answer depends on where evaluation is.
        VariableOf = ("variable_of", "i", PointDependent, 476);
        ReturnTypeOf = ("return_type_of", "i", Stable, 476);
        IsFunctionParameter = ("is_function_parameter", "i", Stable, 476);
        IsExplicitObjectParameter = ("is_explicit_object_parameter", "i", Stable, 476);
        /// A later redeclaration can add a default argument.
        HasDefaultArgument = ("has_default_argument", "i", PointDependent, 476);
        /// The IS's `has_ellipsis_parameter`, under GCC 16.1's name.
        IsVarargFunction = ("is_vararg_function", "i", Stable, 476);
        IsPublic = ("is_public", "i", Stable, 476);
        IsProtected = ("is_protected", "i", Stable, 476);
        IsPrivate = ("is_private", "i", Stable, 476);
        IsVirtual = ("is_virtual", "i", Stable, 476);
        IsPureVirtual = ("is_pure_virtual", "i", Stable, 476);
        IsOverride = ("is_override", "i", Stable, 476);
        IsFinal = ("is_final", "i", Stable, 476);
        IsDeleted = ("is_deleted", "i", Stable, 476);
        /// An out-of-line `= default` redeclaration can come later.
        IsDefaulted = ("is_defaulted", "i", PointDependent, 476);
        IsUserProvided = ("is_user_provided", "i", Stable, 476);
        IsUserDeclared = ("is_user_declared", "i", Stable, 476);
        IsExplicit = ("is_explicit", "i", Stable, 476);
        IsNoexcept = ("is_noexcept", "i", Stable, 476);
        IsBitField = ("is_bit_field", "i", Stable, 476);
        IsEnumerator = ("is_enumerator", "i", Stable, 476);
        IsConst = ("is_const", "i", Stable, 476);
        IsVolatile = ("is_volatile", "i", Stable, 476);
        IsMutableMember = ("is_mutable_member", "i", Stable, 476);
        IsLvalueReferenceQualified = ("is_lvalue_reference_qualified", "i", Stable, 476);
        IsRvalueReferenceQualified = ("is_rvalue_reference_qualified", "i", Stable, 476);
        HasStaticStorageDuration = ("has_static_storage_duration", "i", Stable, 476);
        HasThreadStorageDuration = ("has_thread_storage_duration", "i", Stable, 476);
        HasAutomaticStorageDuration = ("has_automatic_storage_duration", "i", Stable, 476);
        HasInternalLinkage = ("has_internal_linkage", "i", Stable, 476);
        HasModuleLinkage = ("has_module_linkage", "i", Stable, 476);
        HasExternalLinkage = ("has_external_linkage", "i", Stable, 476);
        HasCLanguageLinkage = ("has_c_language_linkage", "i", Stable, 476);
        HasLinkage = ("has_linkage", "i", Stable, 476);
        /// A class or enumeration can be completed later.
        IsCompleteType = ("is_complete_type", "i", PointDependent, 476);
        /// An enumeration can be completed later.
        IsEnumerableType = ("is_enumerable_type", "i", PointDependent, 476);
        IsVariable = ("is_variable", "i", Stable, 476);
        IsType = ("is_type", "i", Stable, 476);
        IsNamespace = ("is_namespace", "i", Stable, 476);
        IsTypeAlias = ("is_type_alias", "i", Stable, 476);
        IsNamespaceAlias = ("is_namespace_alias", "i", Stable, 476);
        IsFunction = ("is_function", "i", Stable, 476);
        IsConversionFunction = ("is_conversion_function", "i", Stable, 476);
        IsOperatorFunction = ("is_operator_function", "i", Stable, 476);
        IsLiteralOperator = ("is_literal_operator", "i", Stable, 476);
        IsSpecialMemberFunction = ("is_special_member_function", "i", Stable, 476);
        IsConstructor = ("is_constructor", "i", Stable, 476);
        IsDefaultConstructor = ("is_default_constructor", "i", Stable, 476);
        IsCopyConstructor = ("is_copy_constructor", "i", Stable, 476);
        IsMoveConstructor = ("is_move_constructor", "i", Stable, 476);
        IsAssignment = ("is_assignment", "i", Stable, 476);
        IsCopyAssignment = ("is_copy_assignment", "i", Stable, 476);
        IsMoveAssignment = ("is_move_assignment", "i", Stable, 476);
        IsDestructor = ("is_destructor", "i", Stable, 476);
        IsTemplate = ("is_template", "i", Stable, 476);
        IsFunctionTemplate = ("is_function_template", "i", Stable, 476);
        IsVariableTemplate = ("is_variable_template", "i", Stable, 476);
        IsClassTemplate = ("is_class_template", "i", Stable, 476);
        IsAliasTemplate = ("is_alias_template", "i", Stable, 476);
        IsConversionFunctionTemplate = ("is_conversion_function_template", "i", Stable, 476);
        IsOperatorFunctionTemplate = ("is_operator_function_template", "i", Stable, 476);
        IsLiteralOperatorTemplate = ("is_literal_operator_template", "i", Stable, 476);
        IsConstructorTemplate = ("is_constructor_template", "i", Stable, 476);
        IsConcept = ("is_concept", "i", Stable, 476);
        IsStructuredBinding = ("is_structured_binding", "i", Stable, 476);
        IsValue = ("is_value", "i", Stable, 476);
        IsObject = ("is_object", "i", Stable, 476);
        IsClassMember = ("is_class_member", "i", Stable, 476);
        IsNamespaceMember = ("is_namespace_member", "i", Stable, 476);
        IsNonstaticDataMember = ("is_nonstatic_data_member", "i", Stable, 476);
        IsStaticMember = ("is_static_member", "i", Stable, 476);
        IsBase = ("is_base", "i", Stable, 476);
        HasDefaultMemberInitializer = ("has_default_member_initializer", "i", Stable, 476);
        HasParent = ("has_parent", "i", Stable, 476);
        /// `std::meta::__kiln_reflection_range(r)`: a `vector<info>` of
        /// the `reflection_range` `r` (the range-argument and vector-result
        /// recipes under test; spec/reflection.md §3.5).
        KilnReflectionRange = ("__kiln_reflection_range", "x", Stable, 475);
        /// `std::meta::__kiln_member_offset(bytes, bits)`: a
        /// `member_offset` (the pair recipe under test, until WP-478's
        /// `offset_of`).
        KilnMemberOffset = ("__kiln_member_offset", "xx", Stable, 475);
        /// `std::meta::__kiln_access_context(scope, designating)`: an
        /// `access_context` (the pair recipe with `info` members).
        KilnAccessContext = ("__kiln_access_context", "ii", Stable, 475);
    }
}
