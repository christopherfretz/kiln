// C++11 and C++14 [temp.arg.nontype]/1: a pointer to member argument is
// "a pointer to member expressed as described in 5.3.1" (`&C::m`), the
// name of a non-type template parameter, or a null member pointer value;
// a constexpr variable holding a pointer to member is not one (clang 22
// "is not a pointer to member constant", GCC 16.1 rejects both). C++17
// allows any converted constant expression
// (temp.arg.nontype/address-forms-accepted-cxx17.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++14
//@ diag: 25:4 error "non-type template argument is not a pointer to member constant"
//@ diag: 26:4 error "non-type template argument is not a pointer to member constant"
//@ diag: 27:5 error "non-type template argument is not a pointer to member constant"
//@ diag: 28:4 error "non-type template argument is not a pointer to member constant"
//@ diag: 29:4 error "non-type template argument is not a pointer to member constant"

struct S { int m; int n; void f(); };
template <int S::*pm> struct PM {};
template <void (S::*pf)()> struct PF {};
constexpr int S::*cp = &S::m;
constexpr void (S::*cf)() = &S::f;
constexpr int S::*pick(bool b) { return b ? &S::m : &S::n; }

PM<cp> e1;
PF<cf> e2;
PM<(cp)> e3;
PM<pick(true)> e4;
PM<true ? &S::m : &S::n> e5;

PM<&S::m> ok1;
PM<(&S::m)> ok2;
PF<&S::f> ok3;
PM<nullptr> ok4;
PM<(int S::*)0> ok5;
template <int S::*q> struct T { PM<q> a; PM<(q)> b; };
T<&S::m> ok6;
T<nullptr> ok7;
