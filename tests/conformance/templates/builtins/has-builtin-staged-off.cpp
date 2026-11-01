// __has_builtin in the modes that run sema answers 0 for the rows kiln
// has not implemented (spec/sema-templates.md §9, Q10), so libc++ uses its
// library fallbacks. WP-166 implemented the last staged-off trait rows
// (they are in has-builtin-staged.cpp now). __has_constexpr_builtin
// answers 0 for a function whose Oven side is a stub (as clang 19 does
// for __builtin_bit_cast).
//@ strict-diags
#if __has_constexpr_builtin(__builtin_bit_cast)
#error constexpr
#endif
