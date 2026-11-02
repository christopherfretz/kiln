// [expr.const]/4, [dcl.array]/1 (clang `EvaluateAsRValue`, as a folded
// namespace-scope array bound uses it): a lax fold reads a `const`
// variable that is not usable in constant expressions (a `const double`,
// a `const` class or array) through a constexpr pointer, reference or
// pointer member to it, as clang folds it; a strict evaluation through
// the same pointer still diagnoses the read (spec/oven.md "Lax folds").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 46:13 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 36:14 note "declared here"
//@ diag: 46:5 warning "variable length array folded to constant array as an extension"
//@ diag: 47:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 47:13 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 36:14 note "declared here"
//@ diag: 47:5 warning "variable length array folded to constant array as an extension"
//@ diag: 48:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 48:13 note "read of non-constexpr variable 'gp' is not allowed in a constant expression"
//@ diag: 38:9 note "declared here"
//@ diag: 48:5 warning "variable length array folded to constant array as an extension"
//@ diag: 49:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 49:13 note "read of non-constexpr variable 'fs' is not allowed in a constant expression"
//@ diag: 39:13 note "declared here"
//@ diag: 49:5 warning "variable length array folded to constant array as an extension"
//@ diag: 50:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 50:13 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 36:14 note "declared here"
//@ diag: 50:5 warning "variable length array folded to constant array as an extension"
//@ diag: 51:19 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 51:24 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 36:14 note "declared here"
//@ diag: 51:17 warning "variable length array folded to constant array as an extension"
//@ diag: 58:18 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 58:22 note "read of non-constexpr variable 'd' is not allowed in a constant expression"
//@ diag: 36:14 note "declared here"
const double d = 5.0;
struct P { double v; int n; };
const P gp = {7.0, 3};
const float fs[2] = {1.0f, 6.0f};
constexpr const double *pd = &d;
constexpr const double &rd = d;
constexpr const P *pp = &gp;
constexpr const float *pf = &fs[1];
struct H { const double *p; };
constexpr H h = {&d};
int a1[(int)*pd];
int a2[(int)rd];
int a3[(int)pp->v + pp->n];
int a4[(int)*pf];
int a5[(int)*h.p];
struct M { char m[(int)*pd]; };
static_assert(sizeof(a1) == 5 * sizeof(int), "");
static_assert(sizeof(a2) == 5 * sizeof(int), "");
static_assert(sizeof(a3) == 10 * sizeof(int), "");
static_assert(sizeof(a4) == 6 * sizeof(int), "");
static_assert(sizeof(a5) == 5 * sizeof(int), "");
static_assert(sizeof(M) == 5, "");
constexpr double v = *pd;
