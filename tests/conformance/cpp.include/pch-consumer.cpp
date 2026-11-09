// [cpp.include]: a precompiled header is a build-system convenience
// (clang's -include-pch), so the test file sees exactly the declarations
// and macros of the header it was built from, entered once: the header has
// no guard, so a second entry would be a redefinition (WP-1023).
//@ expect: compile-ok
//@ strict-diags
//@ pch: Inputs/pch_unguarded.h
constexpr PchPoint p{1, 2};
static_assert(p.sum() == 3);
static_assert(pch_value == 42);
static_assert(pch_twice(2.5) == 5.0);
static_assert(PCH_MACRO(pch_value) == 126);
#ifndef PCH_MACRO
#error "the header's macros are visible after -include-pch"
#endif
