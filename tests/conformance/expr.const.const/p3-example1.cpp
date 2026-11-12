// [expr.const.const]/3 (P4101R1) Example 1, the lines kiln implements:
// every immediate object is a constexpr variable's object, a template
// parameter object, or lives within a constant evaluation. `d` holds a
// non-null reflection (a consteval-only value, /1) and is not constexpr;
// the template parameter object of `C<^^char>` is fine. Lines `a`, `b`
// and `c` (a pointer to an immediate function) are in
// p3-example1-immediate-function.cpp. Compared with GCC 16.1, which
// gives the same verdict under the P2996R13 type rule.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 20:6 error "'d' is initialized with a consteval-only value but is not declared 'constexpr'"
template <auto V> struct C {};

constexpr auto r = ^^int;               // OK: a constexpr variable
auto e = C<^^char>();                   // OK: a template parameter object
static_assert(r == ^^int);              // OK: within a core constant expression
auto d = ^^int;                         // error: not a constexpr variable's object
