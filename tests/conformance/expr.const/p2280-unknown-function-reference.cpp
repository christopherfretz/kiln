// [expr.const]/10 (P2280R4): a reference to a function that is not
// usable in constant expressions is treated as binding to an unspecified
// function: naming it is a constant expression, calling it or comparing
// its address is not. clang 22 agrees, notes included. Found running
// GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ requires: clang>=22
//@ args: -Wno-tautological-undefined-compare
//@ diag: 24:19 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 25:16 error "constexpr variable 'y' must be initialized by a constant expression"
//@ diag: 25:26 note "comparison between pointers to unrelated objects '&fref' and '&f' has unspecified value"
//@ diag: 26:16 error "constexpr variable 'z' must be initialized by a constant expression"
//@ diag: 26:26 note "comparison between pointers to unrelated objects '&fref' and 'nullptr' has unspecified value"
//@ diag: 30:18 error "constexpr variable 'w' must be initialized by a constant expression"
//@ diag: 30:26 note "comparison between pointers to unrelated objects '&pf' and '&f' has unspecified value"
//@ diag: 31:19 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 31:19 note "pointer to 'pf' is not a constant expression"
//@ diag: 29:14 note "declared here"
int f();
extern int (&fref)();
int (&gref)() = f;
constexpr int x = fref();
constexpr bool y = &fref == &f;
constexpr bool z = &fref != nullptr;
constexpr bool k = &gref == &f;
static_assert(k);
void g(int (&pf)()) {
  constexpr bool w = &pf == &f;
  constexpr int (*q)() = &pf;
}
