// Before C++23 a constexpr function whose offsetof index can never be a
// constant is ill-formed, no diagnostic required ([dcl.constexpr]/6);
// clang diagnoses it at the definition. One whose index is a parameter
// is fine, and folds when a call is constant-evaluated; a template's
// index is checked at instantiation.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 15:25 error "constexpr function never produces a constant expression"
//@ diag: 15:62 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 13:5 note "declared here"
struct S { int n; char a[8][4]; };
int k;
constexpr unsigned long f(int i) { return __builtin_offsetof(S, a[i][1]); }
constexpr unsigned long h() { return __builtin_offsetof(S, a[k][1]); }
static_assert(f(2) == 13);
template <int N> unsigned long t(int i) { return __builtin_offsetof(S, a[N][i]); }
unsigned long u = t<3>(1);
