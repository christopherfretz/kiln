// __has_builtin answers 1 for __is_constructible and
// __is_nothrow_constructible (spec/sema-templates.md §9.1), as clang 19 does.
//@ strict-diags
#if !__has_builtin(__is_constructible) || !__has_builtin(__is_nothrow_constructible)
#error constructible traits
#endif
