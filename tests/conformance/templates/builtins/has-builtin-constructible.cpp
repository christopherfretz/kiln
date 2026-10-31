// __has_builtin answers 1 for __is_constructible, __is_nothrow_constructible,
// __is_convertible, __is_convertible_to, __is_nothrow_convertible and
// __is_nothrow_assignable
// (spec/sema-templates.md §9.1), as clang 19 does.
//@ strict-diags
#if !__has_builtin(__is_constructible) || !__has_builtin(__is_nothrow_constructible)
#error constructible traits
#endif
#if !__has_builtin(__is_convertible) || !__has_builtin(__is_convertible_to) || \
    !__has_builtin(__is_nothrow_convertible)
#error convertible traits
#endif
#if !__has_builtin(__is_nothrow_assignable)
#error __is_nothrow_assignable
#endif
