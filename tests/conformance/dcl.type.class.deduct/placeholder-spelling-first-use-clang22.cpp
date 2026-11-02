// [dcl.type.class.deduct]/1: the template named by a deduced class type
// placeholder is printed as written at each use under clang 22 (not as first
// spelled): before and after a later deduction guide, after an explicit
// specialization, through an alias template, inside its own namespace.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:17 error "no viable constructor or deduction guide for deduction of template arguments of 'S::U'"
//@ diag: 56:40 note "candidate template ignored: could not match 'S::U<T>' against 'int'"
//@ diag: 56:40 note "implicit deduction guide declared as 'template <class T> U(S::U<T>) -> S::U<T>'"
//@ diag: 56:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 56:44 note "implicit deduction guide declared as 'template <class T> U(T, int) -> S::U<T>'"
//@ diag: 59:18 error "no viable constructor or deduction guide for deduction of template arguments of 'S::U'"
//@ diag: 56:40 note "candidate template ignored: could not match 'S::U<T>' against 'int'"
//@ diag: 56:40 note "implicit deduction guide declared as 'template <class T> U(S::U<T>) -> S::U<T>'"
//@ diag: 58:33 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 56:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 56:44 note "implicit deduction guide declared as 'template <class T> U(T, int) -> S::U<T>'"
//@ diag: 61:13 warning "expression result unused [-Wunused-value]"
//@ diag: 62:18 error "no viable constructor or deduction guide for deduction of template arguments of 'R::Q'"
//@ diag: 60:40 note "candidate template ignored: could not match 'R::Q<T>' against 'int'"
//@ diag: 60:40 note "implicit deduction guide declared as 'template <class T> Q(R::Q<T>) -> R::Q<T>'"
//@ diag: 60:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 60:44 note "implicit deduction guide declared as 'template <class T> Q(T, int) -> R::Q<T>'"
//@ diag: 65:16 error "no viable constructor or deduction guide for deduction of template arguments of 'AZ'"
//@ diag: 64:19 note "candidate template ignored: could not match 'P::Z<T>' against 'int'"
//@ diag: 64:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(AZ, P::Z<T>) AZ(P::Z<T>) -> P::Z<T>'"
//@ diag: 64:19 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 64:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(AZ, P::Z<T>) AZ(T, int) -> P::Z<T>'"
//@ diag: 65:27 error "no viable constructor or deduction guide for deduction of template arguments of 'P::Z'"
//@ diag: 63:40 note "candidate template ignored: could not match 'P::Z<T>' against 'int'"
//@ diag: 63:40 note "implicit deduction guide declared as 'template <class T> Z(P::Z<T>) -> P::Z<T>'"
//@ diag: 63:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 63:44 note "implicit deduction guide declared as 'template <class T> Z(T, int) -> P::Z<T>'"
//@ diag: 66:71 error "no viable constructor or deduction guide for deduction of template arguments of 'U'"
//@ diag: 66:40 note "candidate template ignored: could not match 'J::U<T>' against 'int'"
//@ diag: 66:40 note "implicit deduction guide declared as 'template <class T> U(J::U<T>) -> J::U<T>'"
//@ diag: 66:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 66:44 note "implicit deduction guide declared as 'template <class T> U(T, int) -> J::U<T>'"
//@ diag: 68:51 error "no viable constructor or deduction guide for deduction of template arguments of 'U2'"
//@ diag: 67:41 note "candidate template ignored: could not match 'J2::U2<T>' against 'int'"
//@ diag: 67:41 note "implicit deduction guide declared as 'template <class T> U2(J2::U2<T>) -> J2::U2<T>'"
//@ diag: 67:46 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 67:46 note "implicit deduction guide declared as 'template <class T> U2(T, int) -> J2::U2<T>'"
//@ diag: 70:3 error "no viable constructor or deduction guide for deduction of template arguments of 'G'"
//@ diag: 69:26 note "candidate template ignored: could not match 'G<T>' against 'int'"
//@ diag: 69:26 note "implicit deduction guide declared as 'template <class T> G(G<T>) -> G<T>'"
//@ diag: 69:30 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 69:30 note "implicit deduction guide declared as 'template <class T> G(T, int) -> G<T>'"
//@ diag: 73:3 error "no viable constructor or deduction guide for deduction of template arguments of 'Y'"
//@ diag: 71:40 note "candidate template ignored: could not match 'X::Y<T>' against 'int'"
//@ diag: 71:40 note "implicit deduction guide declared as 'template <class T> Y(X::Y<T>) -> X::Y<T>'"
//@ diag: 71:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 71:44 note "implicit deduction guide declared as 'template <class T> Y(T, int) -> X::Y<T>'"
//@ requires: clang>=22
// clang 22 spelling of placeholder-spelling-first-use.cpp (WP-377).
namespace S { template<class T> struct U { U(T, int); }; }
void t() { S::U u(1); }
namespace S { template<class T> U(T*) -> U<T>; }
void t2() { S::U u(1); }
namespace R { template<class T> struct Q { Q(T, int); }; }
void t3() { sizeof(R::Q<int>); }
void t4() { R::Q q(1); }
namespace P { template<class T> struct Z { Z(T, int); }; }
template<class T> using AZ = P::Z<T>;
void t5() { AZ z(1); P::Z z2(1); }
namespace J { template<class T> struct U { U(T, int); }; void f() { U u(1); } }
namespace J2 { template<class T> struct U2 { U2(T, int); }; }
namespace J2 { inline namespace I { void f() { U2 u(1); } } }
template<class T> struct G { G(T, int); };
G g(1);
namespace X { template<class T> struct Y { Y(T, int); }; }
using X::Y;
Y y(1);
