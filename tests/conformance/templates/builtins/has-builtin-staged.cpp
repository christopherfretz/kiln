// __has_builtin in the modes that run sema answers from TABLE's
// implemented column (spec/sema-templates.md §9, Q10): every row kiln
// implements answers 1, as clang 19 does. The rows staged off are
// has-builtin-staged-off.cpp.
//@ strict-diags
#if !__has_builtin(__add_lvalue_reference)
#error __add_lvalue_reference
#endif
#if !__has_builtin(__add_pointer)
#error __add_pointer
#endif
#if !__has_builtin(__add_rvalue_reference)
#error __add_rvalue_reference
#endif
#if !__has_builtin(__array_extent)
#error __array_extent
#endif
#if !__has_builtin(__array_rank)
#error __array_rank
#endif
#if !__has_builtin(__builtin_addressof)
#error __builtin_addressof
#endif
#if !__has_builtin(__builtin_bit_cast)
#error __builtin_bit_cast
#endif
#if !__has_builtin(__builtin_is_constant_evaluated)
#error __builtin_is_constant_evaluated
#endif
#if !__has_builtin(__decay)
#error __decay
#endif
#if !__has_builtin(__has_trivial_assign)
#error __has_trivial_assign
#endif
#if !__has_builtin(__has_trivial_constructor)
#error __has_trivial_constructor
#endif
#if !__has_builtin(__has_trivial_copy)
#error __has_trivial_copy
#endif
#if !__has_builtin(__has_trivial_destructor)
#error __has_trivial_destructor
#endif
#if !__has_builtin(__has_trivial_move_assign)
#error __has_trivial_move_assign
#endif
#if !__has_builtin(__has_trivial_move_constructor)
#error __has_trivial_move_constructor
#endif
#if !__has_builtin(__has_unique_object_representations)
#error __has_unique_object_representations
#endif
#if !__has_builtin(__has_virtual_destructor)
#error __has_virtual_destructor
#endif
#if !__has_builtin(__is_abstract)
#error __is_abstract
#endif
#if !__has_builtin(__is_aggregate)
#error __is_aggregate
#endif
#if !__has_builtin(__is_arithmetic)
#error __is_arithmetic
#endif
#if !__has_builtin(__is_array)
#error __is_array
#endif
#if !__has_builtin(__is_base_of)
#error __is_base_of
#endif
#if !__has_builtin(__is_bounded_array)
#error __is_bounded_array
#endif
#if !__has_builtin(__is_class)
#error __is_class
#endif
#if !__has_builtin(__is_complete_type)
#error __is_complete_type
#endif
#if !__has_builtin(__is_compound)
#error __is_compound
#endif
#if !__has_builtin(__is_const)
#error __is_const
#endif
#if !__has_builtin(__is_destructible)
#error __is_destructible
#endif
#if !__has_builtin(__is_empty)
#error __is_empty
#endif
#if !__has_builtin(__is_enum)
#error __is_enum
#endif
#if !__has_builtin(__is_final)
#error __is_final
#endif
#if !__has_builtin(__is_floating_point)
#error __is_floating_point
#endif
#if !__has_builtin(__is_function)
#error __is_function
#endif
#if !__has_builtin(__is_fundamental)
#error __is_fundamental
#endif
#if !__has_builtin(__is_integral)
#error __is_integral
#endif
#if !__has_builtin(__is_literal)
#error __is_literal
#endif
#if !__has_builtin(__is_literal_type)
#error __is_literal_type
#endif
#if !__has_builtin(__is_lvalue_expr)
#error __is_lvalue_expr
#endif
#if !__has_builtin(__is_lvalue_reference)
#error __is_lvalue_reference
#endif
#if !__has_builtin(__is_member_function_pointer)
#error __is_member_function_pointer
#endif
#if !__has_builtin(__is_member_object_pointer)
#error __is_member_object_pointer
#endif
#if !__has_builtin(__is_member_pointer)
#error __is_member_pointer
#endif
#if !__has_builtin(__is_nothrow_destructible)
#error __is_nothrow_destructible
#endif
#if !__has_builtin(__is_nullptr)
#error __is_nullptr
#endif
#if !__has_builtin(__is_object)
#error __is_object
#endif
#if !__has_builtin(__is_pod)
#error __is_pod
#endif
#if !__has_builtin(__is_pointer)
#error __is_pointer
#endif
#if !__has_builtin(__is_polymorphic)
#error __is_polymorphic
#endif
#if !__has_builtin(__is_reference)
#error __is_reference
#endif
#if !__has_builtin(__is_referenceable)
#error __is_referenceable
#endif
#if !__has_builtin(__is_rvalue_expr)
#error __is_rvalue_expr
#endif
#if !__has_builtin(__is_rvalue_reference)
#error __is_rvalue_reference
#endif
#if !__has_builtin(__is_same)
#error __is_same
#endif
#if !__has_builtin(__is_same_as)
#error __is_same_as
#endif
#if !__has_builtin(__is_scalar)
#error __is_scalar
#endif
#if !__has_builtin(__is_scoped_enum)
#error __is_scoped_enum
#endif
#if !__has_builtin(__is_signed)
#error __is_signed
#endif
#if !__has_builtin(__is_standard_layout)
#error __is_standard_layout
#endif
#if !__has_builtin(__is_trivial)
#error __is_trivial
#endif
#if !__has_builtin(__is_trivially_copyable)
#error __is_trivially_copyable
#endif
#if !__has_builtin(__is_trivially_destructible)
#error __is_trivially_destructible
#endif
#if !__has_builtin(__is_unbounded_array)
#error __is_unbounded_array
#endif
#if !__has_builtin(__is_union)
#error __is_union
#endif
#if !__has_builtin(__is_unsigned)
#error __is_unsigned
#endif
#if !__has_builtin(__is_void)
#error __is_void
#endif
#if !__has_builtin(__is_volatile)
#error __is_volatile
#endif
#if !__has_builtin(__make_integer_seq)
#error __make_integer_seq
#endif
#if !__has_builtin(__make_signed)
#error __make_signed
#endif
#if !__has_builtin(__make_unsigned)
#error __make_unsigned
#endif
#if !__has_builtin(__remove_all_extents)
#error __remove_all_extents
#endif
#if !__has_builtin(__remove_const)
#error __remove_const
#endif
#if !__has_builtin(__remove_cv)
#error __remove_cv
#endif
#if !__has_builtin(__remove_cvref)
#error __remove_cvref
#endif
#if !__has_builtin(__remove_extent)
#error __remove_extent
#endif
#if !__has_builtin(__remove_pointer)
#error __remove_pointer
#endif
#if !__has_builtin(__remove_reference_t)
#error __remove_reference_t
#endif
#if !__has_builtin(__remove_restrict)
#error __remove_restrict
#endif
#if !__has_builtin(__remove_volatile)
#error __remove_volatile
#endif
#if !__has_builtin(__underlying_type)
#error __underlying_type
#endif
#if !__has_constexpr_builtin(__builtin_is_constant_evaluated) || !__has_constexpr_builtin(__builtin_addressof)
#error constexpr
#endif
