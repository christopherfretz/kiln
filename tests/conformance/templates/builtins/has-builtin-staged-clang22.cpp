// __has_builtin answers 1 for the builtins clang 20-22 added that libc++
// 22 probes once kiln implements them (WP-1740: __builtin_common_type,
// __builtin_invoke, __builtin_structured_binding_size and the
// __builtin_*_synthesizes_from_spaceship traits), as clang 22 does, and
// __builtin_invoke is a constexpr builtin. Under -fclang-compat=19 they
// answer 0, as clang 19 has none of them (kiln_pp's feature tests).
//@ requires: clang>=22
//@ strict-diags
#if !__has_builtin(__builtin_common_type)
#error __builtin_common_type
#endif
#if !__has_builtin(__builtin_invoke) || !__has_constexpr_builtin(__builtin_invoke)
#error __builtin_invoke
#endif
#if !__has_builtin(__builtin_structured_binding_size)
#error __builtin_structured_binding_size
#endif
#if !__has_builtin(__builtin_lt_synthesizes_from_spaceship)
#error __builtin_lt_synthesizes_from_spaceship
#endif
#if !__has_builtin(__builtin_le_synthesizes_from_spaceship)
#error __builtin_le_synthesizes_from_spaceship
#endif
#if !__has_builtin(__builtin_gt_synthesizes_from_spaceship)
#error __builtin_gt_synthesizes_from_spaceship
#endif
#if !__has_builtin(__builtin_ge_synthesizes_from_spaceship)
#error __builtin_ge_synthesizes_from_spaceship
#endif
