// [over.match.class.deduct]/1: deduction fails when no guide is viable;
// each implicit guide's note says what it was synthesized as.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:6 error "no viable constructor or deduction guide for deduction of template arguments of 'N::S'"
//@ diag: 19:30 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double')"
//@ diag: 19:30 note "implicit deduction guide declared as 'template <class T> S(T, T) -> S<T>'"
//@ diag: 19:26 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 19:26 note "implicit deduction guide declared as 'template <class T> S(S<T>) -> S<T>'"
//@ diag: 24:3 error "no viable constructor or deduction guide for deduction of template arguments of 'U'"
//@ diag: 21:30 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:30 note "implicit deduction guide declared as 'template <class T> U(T) -> U<T>'"
//@ diag: 21:26 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:26 note "implicit deduction guide declared as 'template <class T> U(U<T>) -> U<T>'"
//@ diag: 22:19 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ requires: clang<22
// clang 19 spelling; over.match.class.deduct-no-viable-diag-clang22.cpp has clang 22's (WP-377).
namespace N {
template<class T> struct S { S(T, T); };
}
template<class T> struct U { U(T); };
template<class T> U(T *) -> U<T>;
N::S s(1, 2.0);
U u(1, 2);
