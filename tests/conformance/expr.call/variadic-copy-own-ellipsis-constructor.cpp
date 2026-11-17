// [expr.call]/12, [conv.lval]/3.2: a class glvalue passed through an
// ellipsis is copied by copy-initialization. When the class's only
// viable constructor for that source is a variadic one (a `volatile`
// source binds no implicit copy constructor), the copy passes the object
// through the ellipsis again, without end: kiln rejects it (GCC rejects
// it too, at the same place; clang 22 overflows its stack, hence the
// skip). Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++17
//@ skip(clang): clang 22 and 19 overflow their stack on this test
//@ strict-diags
//@ diag: 21:51 error "cannot pass object of type 'const volatile S' through an ellipsis: its copy selects a variadic constructor that would copy it through the ellipsis again"
//@ diag: 22:49 error "cannot pass object of type 'volatile U<int>' through an ellipsis: its copy selects a variadic constructor that would copy it through the ellipsis again"
//@ diag: 23:28 error "cannot pass object of type 'volatile S' through an ellipsis: its copy selects a variadic constructor that would copy it through the ellipsis again"
//@ diag: 24:34 error "cannot pass object of type 'volatile S' through an ellipsis: its copy selects a variadic constructor that would copy it through the ellipsis again"
struct S { S(...); };
template <class T> struct U { U(...); };
struct D { D(...); D(const D &) = delete; };
void g(...);
// Lines 21-24.
void f1(const volatile S &a) { const volatile S b(a); }
void f2(volatile U<int> &a) { volatile U<int> b(a); }
void f3(volatile S &a) { g(a); }
void f4(volatile S &a) { S b = S(a); }
// A non-volatile source binds the implicit copy constructor: no ellipsis.
void f5(S &a) { S b(a); g(a); }
// A deleted copy constructor is still the better match.
void f6(D &a) { D b(1, 2); (void)b; }
