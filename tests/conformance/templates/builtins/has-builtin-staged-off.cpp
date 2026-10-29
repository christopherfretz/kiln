// __has_builtin in the modes that run sema answers 0 for the rows kiln
// has not implemented (spec/sema-templates.md §9, Q10), so libc++ uses its
// library fallbacks; each row names its owner in TABLE. clang 19 answers 1
// for all of them. __has_constexpr_builtin answers 0 for a function whose
// Oven side is a stub.
//@ strict-diags
//@ xfail(clang): clang 19 implements every row; kiln stages these off
#if __has_builtin(__builtin_FILE)
#error __builtin_FILE
#endif
#if __has_builtin(__builtin_LINE)
#error __builtin_LINE
#endif
#if __has_builtin(__builtin_bswap16)
#error __builtin_bswap16
#endif
#if __has_builtin(__builtin_bswap32)
#error __builtin_bswap32
#endif
#if __has_builtin(__builtin_bswap64)
#error __builtin_bswap64
#endif
#if __has_builtin(__builtin_char_memchr)
#error __builtin_char_memchr
#endif
#if __has_builtin(__builtin_clz)
#error __builtin_clz
#endif
#if __has_builtin(__builtin_clzg)
#error __builtin_clzg
#endif
#if __has_builtin(__builtin_clzl)
#error __builtin_clzl
#endif
#if __has_builtin(__builtin_clzll)
#error __builtin_clzll
#endif
#if __has_builtin(__builtin_constant_p)
#error __builtin_constant_p
#endif
#if __has_builtin(__builtin_ctz)
#error __builtin_ctz
#endif
#if __has_builtin(__builtin_ctzg)
#error __builtin_ctzg
#endif
#if __has_builtin(__builtin_ctzl)
#error __builtin_ctzl
#endif
#if __has_builtin(__builtin_ctzll)
#error __builtin_ctzll
#endif
#if __has_builtin(__builtin_huge_val)
#error __builtin_huge_val
#endif
#if __has_builtin(__builtin_huge_valf)
#error __builtin_huge_valf
#endif
#if __has_builtin(__builtin_huge_vall)
#error __builtin_huge_vall
#endif
#if __has_builtin(__builtin_isfinite)
#error __builtin_isfinite
#endif
#if __has_builtin(__builtin_isgreater)
#error __builtin_isgreater
#endif
#if __has_builtin(__builtin_isgreaterequal)
#error __builtin_isgreaterequal
#endif
#if __has_builtin(__builtin_isinf)
#error __builtin_isinf
#endif
#if __has_builtin(__builtin_isless)
#error __builtin_isless
#endif
#if __has_builtin(__builtin_islessequal)
#error __builtin_islessequal
#endif
#if __has_builtin(__builtin_islessgreater)
#error __builtin_islessgreater
#endif
#if __has_builtin(__builtin_isnan)
#error __builtin_isnan
#endif
#if __has_builtin(__builtin_isnormal)
#error __builtin_isnormal
#endif
#if __has_builtin(__builtin_isunordered)
#error __builtin_isunordered
#endif
#if __has_builtin(__builtin_launder)
#error __builtin_launder
#endif
#if __has_builtin(__builtin_memchr)
#error __builtin_memchr
#endif
#if __has_builtin(__builtin_memcmp)
#error __builtin_memcmp
#endif
#if __has_builtin(__builtin_memcpy)
#error __builtin_memcpy
#endif
#if __has_builtin(__builtin_memmove)
#error __builtin_memmove
#endif
#if __has_builtin(__builtin_nan)
#error __builtin_nan
#endif
#if __has_builtin(__builtin_nanf)
#error __builtin_nanf
#endif
#if __has_builtin(__builtin_nanl)
#error __builtin_nanl
#endif
#if __has_builtin(__builtin_nans)
#error __builtin_nans
#endif
#if __has_builtin(__builtin_nansf)
#error __builtin_nansf
#endif
#if __has_builtin(__builtin_nansl)
#error __builtin_nansl
#endif
#if __has_builtin(__builtin_operator_delete)
#error __builtin_operator_delete
#endif
#if __has_builtin(__builtin_operator_new)
#error __builtin_operator_new
#endif
#if __has_builtin(__builtin_popcount)
#error __builtin_popcount
#endif
#if __has_builtin(__builtin_popcountg)
#error __builtin_popcountg
#endif
#if __has_builtin(__builtin_popcountl)
#error __builtin_popcountl
#endif
#if __has_builtin(__builtin_popcountll)
#error __builtin_popcountll
#endif
#if __has_builtin(__builtin_signbit)
#error __builtin_signbit
#endif
#if __has_builtin(__builtin_strlen)
#error __builtin_strlen
#endif
#if __has_builtin(__builtin_unreachable)
#error __builtin_unreachable
#endif
#if __has_builtin(__builtin_wcslen)
#error __builtin_wcslen
#endif
#if __has_builtin(__builtin_wmemchr)
#error __builtin_wmemchr
#endif
#if __has_builtin(__builtin_wmemcmp)
#error __builtin_wmemcmp
#endif
#if __has_builtin(__can_pass_in_regs)
#error __can_pass_in_regs
#endif
#if __has_builtin(__has_nothrow_assign)
#error __has_nothrow_assign
#endif
#if __has_builtin(__has_nothrow_constructor)
#error __has_nothrow_constructor
#endif
#if __has_builtin(__has_nothrow_copy)
#error __has_nothrow_copy
#endif
#if __has_builtin(__has_nothrow_move_assign)
#error __has_nothrow_move_assign
#endif
#if __has_builtin(__is_assignable)
#error __is_assignable
#endif
#if __has_builtin(__is_bitwise_cloneable)
#error __is_bitwise_cloneable
#endif
#if __has_builtin(__is_constructible)
#error __is_constructible
#endif
#if __has_builtin(__is_convertible)
#error __is_convertible
#endif
#if __has_builtin(__is_convertible_to)
#error __is_convertible_to
#endif
#if __has_builtin(__is_layout_compatible)
#error __is_layout_compatible
#endif
#if __has_builtin(__is_nothrow_assignable)
#error __is_nothrow_assignable
#endif
#if __has_builtin(__is_nothrow_constructible)
#error __is_nothrow_constructible
#endif
#if __has_builtin(__is_nothrow_convertible)
#error __is_nothrow_convertible
#endif
#if __has_builtin(__is_pointer_interconvertible_base_of)
#error __is_pointer_interconvertible_base_of
#endif
#if __has_builtin(__is_trivially_assignable)
#error __is_trivially_assignable
#endif
#if __has_builtin(__is_trivially_constructible)
#error __is_trivially_constructible
#endif
#if __has_builtin(__is_trivially_equality_comparable)
#error __is_trivially_equality_comparable
#endif
#if __has_builtin(__is_trivially_relocatable)
#error __is_trivially_relocatable
#endif
#if __has_builtin(__reference_binds_to_temporary)
#error __reference_binds_to_temporary
#endif
#if __has_builtin(__reference_constructs_from_temporary)
#error __reference_constructs_from_temporary
#endif
#if __has_builtin(__reference_converts_from_temporary)
#error __reference_converts_from_temporary
#endif
#if __has_builtin(__type_pack_element)
#error __type_pack_element
#endif
#if __has_constexpr_builtin(__builtin_bit_cast) || __has_constexpr_builtin(__builtin_memcpy)
#error constexpr
#endif
