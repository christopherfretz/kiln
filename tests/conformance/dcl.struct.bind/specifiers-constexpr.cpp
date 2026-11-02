//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:1 error "declaration cannot be declared 'constexpr'"
//@ diag: 14:1 error "declaration cannot be declared 'constinit'"
//@ diag: 15:1 error "declaration cannot be declared with 'constexpr inline' specifiers"
//@ requires: clang<22
// [dcl.pre]/6 before C++26: constexpr and constinit are not allowed on a
// structured binding declaration. clang 22 implements P2686 (constexpr
// structured bindings) and accepts the first two; kiln does not implement
// P2686 yet, so this stays pinned to clang 19 (WP-377 residual).
struct S { int a, b; };
constexpr S cs{1, 2};
constexpr auto [a1, b1] = cs;
constinit auto [a2, b2] = cs;
constexpr inline auto [a4, b4] = cs;
