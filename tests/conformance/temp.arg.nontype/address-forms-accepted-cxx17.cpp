// C++17 [temp.arg.nontype]/1-2: a non-type template argument is any
// converted constant expression of the parameter's type (not a
// subobject's address), so the forms C++14 rejected are arguments:
// a dereferenced or offset pointer parameter for a reference or
// pointer parameter, a constexpr variable or call for a pointer to
// member (clang 22 accepts, GCC 16.1 too except `q[0]`;
// temp.arg.nontype/address-form-dependent-before-cxx17.cpp,
// temp.arg.nontype/member-pointer-form-before-cxx17.cpp).
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++17

template <int *p> struct R {};
template <int &r> struct RR {};
int x;
template <int *q> struct T1 { RR<*q> a; };
template <int *q> struct T2 { RR<q[0]> a; };
template <int *q> struct T3 { R<q + 0> a; };
template <int *q> struct T4 { R<static_cast<int *>(q)> a; };
template <int *q> struct T5 { R<&*q> a; };
T1<&x> t1;
T2<&x> t2;
T3<&x> t3;
T4<&x> t4;
T5<&x> t5;

struct S { int m; int n; void f(); };
template <int S::*pm> struct PM {};
template <void (S::*pf)()> struct PF {};
constexpr int S::*cp = &S::m;
constexpr void (S::*cf)() = &S::f;
constexpr int S::*pick(bool b) { return b ? &S::m : &S::n; }
PM<cp> m1;
PF<cf> m2;
PM<pick(true)> m3;
PM<true ? &S::m : &S::n> m4;
