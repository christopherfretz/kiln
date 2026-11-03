// [expr.const]/22 (clang `VarDecl::evaluateValue`): a const integral
// variable whose constant initialization fails (it reads non-constant
// `x` when `__builtin_is_constant_evaluated()` is true) is not usable in
// constant expressions, but a fold reads the value of its initializer
// evaluated non-manifestly (9) after noting "initializer of 'cx' is not a
// constant expression": `int a[cx]` is a VLA folded to a constant array
// and `__builtin_constant_p(cx)` is true. `cy`, whose initializer never
// folds, stays a VLA bound; a manifestly constant-evaluated read of
// either is an error. `__builtin_constant_p` folds laxly too ([expr.const]/4
// rejects reading a const double, clang only notes it): d0, d1 and *p1
// are constant as in clang 19 and 22 (spec/oven.md "Lax folds").
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:16 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 33:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 33:7 note "initializer of 'cx' is not a constant expression"
//@ diag: 31:11 note "declared here"
//@ diag: 33:5 warning "variable length array folded to constant array as an extension"
//@ diag: 34:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 34:7 note "initializer of 'cy' is not a constant expression"
//@ diag: 32:11 note "declared here"
//@ diag: 34:5 error "variable length array declaration not allowed at file scope"
//@ diag: 35:15 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 35:19 note "initializer of 'cx' is not a constant expression"
//@ diag: 31:11 note "declared here"
//@ diag: 36:15 error "constexpr variable 'w' must be initialized by a constant expression"
//@ diag: 36:19 note "initializer of 'cy' is not a constant expression"
//@ diag: 32:11 note "declared here"
int x = 1;
const int cx = __builtin_is_constant_evaluated() ? x : 9;
const int cy = x;
int a[cx];
int b[cy];
constexpr int v = cx;
constexpr int w = cy;
static_assert(sizeof(a) == 9 * sizeof(int));
static_assert(__builtin_constant_p(cx));
static_assert(!__builtin_constant_p(cy));
const double d0 = 2.5;
const double d1 = d0 * 2;
constexpr const double *p1 = &d1;
struct S {
  double v;
  int n;
};
const S s = {d1, (int)d0};
static_assert(__builtin_constant_p(d0));
static_assert(__builtin_constant_p(d1));
static_assert(__builtin_constant_p(*p1));
static_assert(__builtin_constant_p(d1 + 1));
static_assert(__builtin_constant_p(s.v));
static_assert(!__builtin_constant_p(x));
int c[__builtin_constant_p(d1) ? 1 : 2];
static_assert(sizeof(c) == sizeof(int));
