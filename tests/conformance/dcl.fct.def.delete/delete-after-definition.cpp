// [dcl.fct.def.delete]/4, [basic.def.odr]/1: '= delete' after a definition
// of the same function (or explicit specialization) redefines it. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 12:18 error "redefinition of 'f<int>'"
//@ diag: 11:18 note "previous definition is here"
//@ diag: 14:6 error "redefinition of 'g'"
//@ diag: 13:6 note "previous definition is here"
template <class T> void f(T);
template <> void f<int>(int) {}
template <> void f<int>(int) = delete;
void g(int) {}
void g(int) = delete;
struct S { void m(); };
void S::m() {}
// Valid: deleted on its first declaration, and declared then defined.
template <> void f<long>(long) = delete;
template <> void f<char>(char);
template <> void f<char>(char) {}
void h(int) = delete;
