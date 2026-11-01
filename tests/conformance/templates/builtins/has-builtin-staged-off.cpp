// __has_builtin in the modes that run sema answers 0 for the rows kiln
// has not implemented (spec/sema-templates.md §9, Q10), so libc++ uses its
// library fallbacks; each row names its owner in TABLE. clang 19 answers 1
// for all of them. __has_constexpr_builtin answers 0 for a function whose
// Oven side is a stub.
//@ strict-diags
//@ xfail(clang): clang 19 implements every row; kiln stages these off
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
#if __has_builtin(__is_bitwise_cloneable)
#error __is_bitwise_cloneable
#endif
#if __has_builtin(__is_layout_compatible)
#error __is_layout_compatible
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
#if __has_builtin(__type_pack_element)
#error __type_pack_element
#endif
#if __has_constexpr_builtin(__builtin_bit_cast)
#error constexpr
#endif
