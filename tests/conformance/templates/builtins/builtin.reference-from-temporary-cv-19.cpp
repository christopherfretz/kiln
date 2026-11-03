// [meta.unary.prop] reference_constructs_from_temporary /
// reference_converts_from_temporary, [expr.type]/2: a `const int` prvalue
// is an `int` prvalue, so it initializes `int&&` through a temporary. kiln
// gives the standard answer at -fclang-compat=19 too (clang 19 keeps the
// cv-qualifiers and answers false: a clang bug, fixed by clang 22), so the
// result does not depend on the oracle or on which <type_traits> is used.
//@ expect: compile-ok
//@ args: -fclang-compat=19
//@ skip(clang): clang 19 keeps the cv-qualifiers of a non-class prvalue here
//@ oracle: gcc
static_assert(__reference_constructs_from_temporary(int&&, const int));
static_assert(__reference_converts_from_temporary(int&&, const int));
static_assert(__reference_constructs_from_temporary(int&&, const volatile int));
static_assert(!__reference_constructs_from_temporary(int&, const int));
int main() {}
