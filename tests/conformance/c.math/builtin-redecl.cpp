// [c.math.syn], [dcl.link]/6: a file-scope `extern "C"` redeclaration of
// a math builtin (`__builtin_sqrt`, …) with another parameter list is
// "conflicting types" with the builtin-type note (clang
// `MergeFunctionDecl`), while the C library names (`sqrt`, `sinf`) are no
// builtins in C++ until declared with their exact type: a declaration
// with another type is an ordinary function and draws no
// incompatible-library-redeclaration warning (that one is C-only).
// spec/sema.md §5.4 (WP-410).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:19 error "conflicting types for '__builtin_sqrt'"
//@ diag: 17:19 note "'__builtin_sqrt' is a builtin with type 'double (double) noexcept'"
//@ diag: 18:18 error "conflicting types for '__builtin_fmaf'"
//@ diag: 18:18 note "'__builtin_fmaf' is a builtin with type 'float (float, float, float) noexcept'"
//@ diag: 19:24 error "conflicting types for '__builtin_roundevenl'"
//@ diag: 19:24 note "'__builtin_roundevenl' is a builtin with type 'long double (long double) noexcept'"
extern "C" double __builtin_sqrt(float);
extern "C" float __builtin_fmaf(float, float);
extern "C" long double __builtin_roundevenl(double);
extern "C" int sqrt(int);
extern "C" double sinf(double);
extern "C" double cos(double);
int s = sqrt(4);
double t = sinf(1.0) + cos(0.0) + __builtin_sqrt(4.0);
