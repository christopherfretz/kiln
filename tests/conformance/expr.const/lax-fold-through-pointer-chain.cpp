//@ expect: compile-fail
//@ strict-diags
//@ diag: 54:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 54:13 note "read of non-constexpr variable 'd1' is not allowed in a constant expression"
//@ diag: 42:14 note "declared here"
//@ diag: 54:5 warning "variable length array folded to constant array as an extension"
//@ diag: 55:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 55:13 note "read of non-constexpr variable 'd2' is not allowed in a constant expression"
//@ diag: 43:21 note "declared here"
//@ diag: 55:5 warning "variable length array folded to constant array as an extension"
//@ diag: 56:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 56:13 note "read of non-constexpr variable 'd3' is not allowed in a constant expression"
//@ diag: 44:21 note "declared here"
//@ diag: 56:5 warning "variable length array folded to constant array as an extension"
//@ diag: 57:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 57:13 note "read of non-constexpr variable 'd1' is not allowed in a constant expression"
//@ diag: 42:14 note "declared here"
//@ diag: 57:5 warning "variable length array folded to constant array as an extension"
//@ diag: 58:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 58:13 note "read of non-constexpr variable 's' is not allowed in a constant expression"
//@ diag: 47:9 note "declared here"
//@ diag: 58:5 warning "variable length array folded to constant array as an extension"
//@ diag: 64:8 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 64:8 note "read of non-const variable 'si' is not allowed in a constant expression"
//@ diag: 45:12 note "declared here"
//@ diag: 64:5 error "variable length array declaration not allowed at file scope"
//@ diag: 65:18 error "constexpr variable 'v1' must be initialized by a constant expression"
//@ diag: 65:23 note "read of non-constexpr variable 'd1' is not allowed in a constant expression"
//@ diag: 42:14 note "declared here"
//@ diag: 66:18 error "constexpr variable 'v2' must be initialized by a constant expression"
//@ diag: 66:23 note "read of non-constexpr variable 'd1' is not allowed in a constant expression"
//@ diag: 42:14 note "declared here"
// [expr.const]/4, [dcl.array]/1 (clang `EvaluateAsRValue`): the address
// of a namespace-scope variable is a constant however its value folds, so
// a constexpr pointer or reference to a `const` variable whose
// initializer itself reads a `const double` (a lax initializer) folds
// through, reading the value clang's lax evaluation of that initializer
// gives; a strict read through the same pointer is still diagnosed, and
// a pointer to a non-`const` variable does not fold (spec/oven.md "Lax
// folds", WP-407).
const double d0 = 2.5;
const double d1 = d0 * 2;
static const double d2 = d1 + 1;
extern const double d3 = d0 + 4;
static int si = (int)d0;
struct S { double v; int n; };
const S s = {d1, (int)d0};
constexpr const double *p1 = &d1;
constexpr const double *p2 = &d2;
constexpr const double *p3 = &d3;
constexpr const double &r1 = d1;
constexpr const S *ps = &s;
constexpr int *pi = &si;
int a1[(int)*p1];
int a2[(int)*p2];
int a3[(int)*p3];
int a4[(int)r1 + 1];
int a5[(int)ps->v + ps->n];
static_assert(sizeof(a1) == 5 * sizeof(int), "");
static_assert(sizeof(a2) == 6 * sizeof(int), "");
static_assert(sizeof(a3) == 6 * sizeof(int), "");
static_assert(sizeof(a4) == 6 * sizeof(int), "");
static_assert(sizeof(a5) == 7 * sizeof(int), "");
int b1[*pi];
constexpr double v1 = *p1;
constexpr double v2 = r1;
static_assert(p1 == &d1 && p2 != p3, "");
