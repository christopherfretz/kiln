// [expr.type.conv]/2: `T()`, `T{}` and `T(a, b)` initialize a prvalue of
// type `T` ([dcl.init.general]/16). A class type must be complete
// ([basic.def]/5, clang `BuildCXXTypeConstructExpr`): "invalid use of
// incomplete type", or only the instantiation's error for an undefined
// class template specialization. An array type cannot be
// value-initialized, a reference needs an initializer
// ([dcl.init.ref]/1) and takes one expression. clang is the oracle.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:24 error "implicit instantiation of undefined template 'cond<false>'"
//@ diag: 39:23 note "template is declared here"
//@ diag: 45:12 error "implicit instantiation of undefined template 'cond<false>'"
//@ diag: 39:23 note "template is declared here"
//@ diag: 46:12 error "invalid use of incomplete type 'I'"
//@ diag: 40:8 note "forward declaration of 'I'"
//@ diag: 47:12 error "invalid use of incomplete type 'I'"
//@ diag: 40:8 note "forward declaration of 'I'"
//@ diag: 48:12 error "implicit instantiation of undefined template 'cond<true>'"
//@ diag: 39:23 note "template is declared here"
//@ diag: 49:12 error "implicit instantiation of undefined template 'cond<true>'"
//@ diag: 39:23 note "template is declared here"
//@ diag: 50:12 error "invalid use of incomplete type 'I'"
//@ diag: 40:8 note "forward declaration of 'I'"
//@ diag: 51:30 error "invalid use of incomplete type 'I'"
//@ diag: 52:15 note "in instantiation of function template specialization 'h<I>' requested here"
//@ diag: 40:8 note "forward declaration of 'I'"
//@ diag: 51:35 error "invalid use of incomplete type 'I'"
//@ diag: 40:8 note "forward declaration of 'I'"
//@ diag: 51:40 error "invalid use of incomplete type 'I'"
//@ diag: 40:8 note "forward declaration of 'I'"
//@ diag: 53:13 error "array types cannot be value-initialized"
//@ diag: 54:13 error "array types cannot be value-initialized"
//@ diag: 55:13 error "reference to type 'int' requires an initializer"
//@ diag: 56:13 error "reference to type 'const int' requires an initializer"
//@ diag: 57:24 error "reference cannot be initialized with multiple values"
//@ diag: 58:13 warning "expression result unused"
//@ diag: 59:17 error "invalid use of incomplete type 'I'"
//@ diag: 40:8 note "forward declaration of 'I'"
template<bool> struct cond;
struct I;
using Arr = int[2];
using R = int&;
using CR = const int&;
void a() { cond<false> c{1}; }
void b() { cond<false>(); }
void c() { I(); }
void d() { I{}; }
void e() { cond<true>{}; }
void f() { cond<true>(1, 2); }
void g() { I(1, 2); }
template<class T> void h() { T(); T{}; T(1, 2); }
template void h<I>();
void a1() { Arr(); }
void a3() { Arr(1, 2); }
void r1() { R(); }
void r2() { CR(); }
void r4() { int i = 0; R(i, i); }
void r5() { CR{}; }
void p(int n) { I(n, n); }
