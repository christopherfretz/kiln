// [temp.mem], [over.match.class.deduct]/1: the implicit guide of a member
// template's constructor prints with the enclosing argument substituted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:6 error "no viable constructor or deduction guide for deduction of template arguments of 'O::Q'"
//@ diag: 12:63 note "candidate function template not viable: no known conversion from 'const char[2]' to 'int' for 2nd argument"
//@ diag: 12:63 note "implicit deduction guide declared as 'template <class U> Q(U, int) -> Q<U>'"
//@ diag: 12:59 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 12:59 note "implicit deduction guide declared as 'template <class U> Q(Q<U>) -> Q<U>'"
//@ requires: clang<22
// clang 22 emits error: no viable constructor or deduction guide for deduction of template argument...; WP-355 residual
template<class T> struct Outer { template<class U> struct Q { Q(U, T); }; };
using O = Outer<int>;
O::Q q(1.0, "x");
