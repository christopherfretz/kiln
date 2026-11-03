//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 17:1 error "declaration cannot be declared 'constexpr'"
//@ diag: 18:1 error "declaration cannot be declared 'constinit'"
//@ diag: 19:1 error "declaration cannot be declared 'inline'"
//@ diag: 20:8 error "declaration cannot be declared 'constexpr'"
// [dcl.pre]/6 before C++26: constexpr and constinit are not allowed on a
// structured binding declaration (P2686R5 allows them from C++26,
// p2686-constexpr.cpp). clang 22 rejects them at -std=c++23 with an error,
// not an extension warning; clang 19's wording says "decomposition
// declaration" (the directives match the common suffix). `constexpr
// inline` is left out: clang 22 reports it as two errors, clang 19 as
// one.
struct S { int a, b; };
constexpr S cs{1, 2};
constexpr auto [a1, b1] = cs;
constinit auto [a2, b2] = cs;
inline auto [a4, b4] = cs;
static constexpr auto [a5, b5] = cs;
