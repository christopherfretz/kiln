// [dcl.fct.def.delete]/2, [expr.type.conv]/2: a functional-style cast
// `A(1)` selecting the deleted `A(int)` is ill-formed, also as the operand
// of decltype; every viable candidate is noted, the deleted one first. In
// a return type it is a substitution failure, so the other overload is
// called. Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:21 error "functional-style cast from 'int' to 'A' uses deleted function"
//@ diag: 12:12 note "candidate constructor has been explicitly deleted"
//@ diag: 12:8 note "candidate constructor (the implicit copy constructor)"
//@ diag: 12:8 note "candidate constructor (the implicit move constructor)"
struct A { A(int) = delete; constexpr A() {} };

template <class T> constexpr int f(T x, decltype(A(x))* = nullptr) { return 1; }
template <class T> constexpr int f(...) { return 2; }
static_assert(f<int>(1) == 2);
static_assert(f<A>(A()) == 1);
using T2 = decltype(A());
static_assert(__is_same_as(T2, A));

using T1 = decltype(A(1));
