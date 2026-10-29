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
#if !__has_builtin(__builtin_COLUMN)
#error __builtin_COLUMN
#endif
#if !__has_builtin(__builtin_FILE)
#error __builtin_FILE
#endif
#if !__has_builtin(__builtin_FILE_NAME)
#error __builtin_FILE_NAME
#endif
#if !__has_builtin(__builtin_FUNCTION)
#error __builtin_FUNCTION
#endif
#if !__has_builtin(__builtin_LINE)
#error __builtin_LINE
#endif
#if !__has_builtin(__builtin_bswap16)
#error __builtin_bswap16
#endif
#if !__has_builtin(__builtin_bswap32)
#error __builtin_bswap32
#endif
#if !__has_builtin(__builtin_bswap64)
#error __builtin_bswap64
#endif
#if !__has_builtin(__builtin_char_memchr)
#error __builtin_char_memchr
#endif
#if !__has_builtin(__builtin_clz)
#error __builtin_clz
#endif
#if !__has_builtin(__builtin_clzg)
#error __builtin_clzg
#endif
#if !__has_builtin(__builtin_clzl)
#error __builtin_clzl
#endif
#if !__has_builtin(__builtin_clzll)
#error __builtin_clzll
#endif
#if !__has_builtin(__builtin_ctz)
#error __builtin_ctz
#endif
#if !__has_builtin(__builtin_ctzg)
#error __builtin_ctzg
#endif
#if !__has_builtin(__builtin_ctzl)
#error __builtin_ctzl
#endif
#if !__has_builtin(__builtin_ctzll)
#error __builtin_ctzll
#endif
#if !__has_builtin(__builtin_exp)
#error __builtin_exp
#endif
#if !__has_builtin(__builtin_exp2)
#error __builtin_exp2
#endif
#if !__has_builtin(__builtin_exp2f)
#error __builtin_exp2f
#endif
#if !__has_builtin(__builtin_exp2l)
#error __builtin_exp2l
#endif
#if !__has_builtin(__builtin_expf)
#error __builtin_expf
#endif
#if !__has_builtin(__builtin_expl)
#error __builtin_expl
#endif
#if !__has_builtin(__builtin_expm1)
#error __builtin_expm1
#endif
#if !__has_builtin(__builtin_expm1f)
#error __builtin_expm1f
#endif
#if !__has_builtin(__builtin_expm1l)
#error __builtin_expm1l
#endif
#if !__has_builtin(__builtin_frexp)
#error __builtin_frexp
#endif
#if !__has_builtin(__builtin_frexpf)
#error __builtin_frexpf
#endif
#if !__has_builtin(__builtin_frexpl)
#error __builtin_frexpl
#endif
#if !__has_builtin(__builtin_huge_val)
#error __builtin_huge_val
#endif
#if !__has_builtin(__builtin_huge_valf)
#error __builtin_huge_valf
#endif
#if !__has_builtin(__builtin_huge_vall)
#error __builtin_huge_vall
#endif
#if !__has_builtin(__builtin_isfinite)
#error __builtin_isfinite
#endif
#if !__has_builtin(__builtin_isgreater)
#error __builtin_isgreater
#endif
#if !__has_builtin(__builtin_isgreaterequal)
#error __builtin_isgreaterequal
#endif
#if !__has_builtin(__builtin_isinf)
#error __builtin_isinf
#endif
#if !__has_builtin(__builtin_isless)
#error __builtin_isless
#endif
#if !__has_builtin(__builtin_islessequal)
#error __builtin_islessequal
#endif
#if !__has_builtin(__builtin_islessgreater)
#error __builtin_islessgreater
#endif
#if !__has_builtin(__builtin_isnan)
#error __builtin_isnan
#endif
#if !__has_builtin(__builtin_isnormal)
#error __builtin_isnormal
#endif
#if !__has_builtin(__builtin_isunordered)
#error __builtin_isunordered
#endif
#if !__has_builtin(__builtin_launder)
#error __builtin_launder
#endif
#if !__has_builtin(__builtin_ldexp)
#error __builtin_ldexp
#endif
#if !__has_builtin(__builtin_ldexpf)
#error __builtin_ldexpf
#endif
#if !__has_builtin(__builtin_ldexpl)
#error __builtin_ldexpl
#endif
#if !__has_builtin(__builtin_memchr)
#error __builtin_memchr
#endif
#if !__has_builtin(__builtin_memcmp)
#error __builtin_memcmp
#endif
#if !__has_builtin(__builtin_memcpy)
#error __builtin_memcpy
#endif
#if !__has_builtin(__builtin_memmove)
#error __builtin_memmove
#endif
#if !__has_builtin(__builtin_nan)
#error __builtin_nan
#endif
#if !__has_builtin(__builtin_nanf)
#error __builtin_nanf
#endif
#if !__has_builtin(__builtin_nanl)
#error __builtin_nanl
#endif
#if !__has_builtin(__builtin_nans)
#error __builtin_nans
#endif
#if !__has_builtin(__builtin_nansf)
#error __builtin_nansf
#endif
#if !__has_builtin(__builtin_nansl)
#error __builtin_nansl
#endif
#if !__has_builtin(__builtin_operator_delete)
#error __builtin_operator_delete
#endif
#if !__has_builtin(__builtin_operator_new)
#error __builtin_operator_new
#endif
#if !__has_builtin(__builtin_popcount)
#error __builtin_popcount
#endif
#if !__has_builtin(__builtin_popcountg)
#error __builtin_popcountg
#endif
#if !__has_builtin(__builtin_popcountl)
#error __builtin_popcountl
#endif
#if !__has_builtin(__builtin_popcountll)
#error __builtin_popcountll
#endif
#if !__has_builtin(__builtin_pow)
#error __builtin_pow
#endif
#if !__has_builtin(__builtin_powf)
#error __builtin_powf
#endif
#if !__has_builtin(__builtin_powl)
#error __builtin_powl
#endif
#if !__has_builtin(__builtin_scalbln)
#error __builtin_scalbln
#endif
#if !__has_builtin(__builtin_scalblnf)
#error __builtin_scalblnf
#endif
#if !__has_builtin(__builtin_scalblnl)
#error __builtin_scalblnl
#endif
#if !__has_builtin(__builtin_scalbn)
#error __builtin_scalbn
#endif
#if !__has_builtin(__builtin_scalbnf)
#error __builtin_scalbnf
#endif
#if !__has_builtin(__builtin_scalbnl)
#error __builtin_scalbnl
#endif
#if !__has_builtin(__builtin_signbit)
#error __builtin_signbit
#endif
#if !__has_builtin(__builtin_source_location)
#error __builtin_source_location
#endif
#if !__has_builtin(__builtin_strlen)
#error __builtin_strlen
#endif
#if !__has_builtin(__builtin_unreachable)
#error __builtin_unreachable
#endif
#if !__has_builtin(__builtin_wcslen)
#error __builtin_wcslen
#endif
#if !__has_builtin(__builtin_wmemchr)
#error __builtin_wmemchr
#endif
#if !__has_builtin(__builtin_wmemcmp)
#error __builtin_wmemcmp
#endif
#if !__has_constexpr_builtin(__builtin_memcpy) || !__has_constexpr_builtin(__builtin_strlen) || !__has_constexpr_builtin(__builtin_popcount)
#error constexpr WP-088
#endif
#if !__has_constexpr_builtin(__builtin_is_constant_evaluated) || !__has_constexpr_builtin(__builtin_addressof)
#error constexpr
#endif
