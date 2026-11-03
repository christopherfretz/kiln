//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 26:11 error "declaration cannot be declared 'inline'"
//@ diag: 27:1 error "declaration cannot be declared 'consteval'"
//@ diag: 27:1 error "consteval can only be used in function declarations"
//@ diag: 29:16 error "constexpr variable '[a6, b6]' must be initialized by a constant expression"
//@ diag: 29:29 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 28:5 note "declared here"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:15 note "read of non-constexpr variable 'ms' is not allowed in a constant expression"
//@ diag: 31:3 note "declared here"
//@ diag: 36:19 error "constexpr variable '[m, n]' must be initialized by a constant expression"
//@ diag: 36:19 note "reference to 'loc' is not a constant expression"
//@ diag: 35:5 note "declared here"
//@ diag: 38:18 error "constexpr variable '[u, v]' must be initialized by a constant expression"
//@ diag: 38:27 note "read of non-constexpr variable 'arr' is not allowed in a constant expression"
//@ diag: 37:7 note "declared here"
// [dcl.pre]/6 (C++26, P2686R5): constexpr and constinit are allowed on a
// structured binding declaration; inline and consteval still are not. A
// constexpr one is constant-initialized ([dcl.constexpr]/6): from a
// non-constant, through a reference to an automatic object, or by copying
// a non-constexpr array, it is ill-formed.
struct S { int a, b; };
constexpr S cs{1, 2};
constexpr inline auto [a4, b4] = cs;
consteval auto [a5, b5] = cs;
int nc = 3;
constexpr auto [a6, b6] = S{nc, 2};
constexpr auto& [a7, b7] = cs;
S ms{1, 2};
constexpr auto& [a8, b8] = ms;
static_assert(a8 == 1);
void h() {
  S loc{1, 2};
  constexpr auto& [m, n] = loc;
  int arr[2] = {1, 2};
  constexpr auto [u, v] = arr;
}
