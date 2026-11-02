// [dcl.type.class.deduct]/1: the template named by a deduced class type
// placeholder is printed as first spelled for that template: before and after
// a later deduction guide, after an explicit specialization, through an alias
// template, inside its own namespace and through a using-declaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:17 error "no viable constructor or deduction guide for deduction of template arguments of 'S::U'"
//@ diag: 56:40 note "candidate template ignored: could not match 'U<T>' against 'int'"
//@ diag: 56:40 note "implicit deduction guide declared as 'template <class T> U(U<T>) -> U<T>'"
//@ diag: 56:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 56:44 note "implicit deduction guide declared as 'template <class T> U(T, int) -> U<T>'"
//@ diag: 59:18 error "no viable constructor or deduction guide for deduction of template arguments of 'S::U'"
//@ diag: 56:40 note "candidate template ignored: could not match 'U<T>' against 'int'"
//@ diag: 56:40 note "implicit deduction guide declared as 'template <class T> U(U<T>) -> U<T>'"
//@ diag: 58:33 note "candidate template ignored: could not match 'T *' against 'int'"
//@ diag: 56:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 56:44 note "implicit deduction guide declared as 'template <class T> U(T, int) -> U<T>'"
//@ diag: 61:13 warning "expression result unused [-Wunused-value]"
//@ diag: 62:18 error "no viable constructor or deduction guide for deduction of template arguments of 'R::Q'"
//@ diag: 60:40 note "candidate template ignored: could not match 'Q<T>' against 'int'"
//@ diag: 60:40 note "implicit deduction guide declared as 'template <class T> Q(Q<T>) -> Q<T>'"
//@ diag: 60:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 60:44 note "implicit deduction guide declared as 'template <class T> Q(T, int) -> Q<T>'"
//@ diag: 65:16 error "no viable constructor or deduction guide for deduction of template arguments of 'AZ'"
//@ diag: 64:19 note "candidate template ignored: could not match 'Z<T>' against 'int'"
//@ diag: 64:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(AZ, Z<T>) AZ(Z<T>) -> Z<T>'"
//@ diag: 64:19 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 64:19 note "implicit deduction guide declared as 'template <class T> requires __is_deducible(AZ, Z<T>) AZ(T, int) -> Z<T>'"
//@ diag: 65:27 error "no viable constructor or deduction guide for deduction of template arguments of 'P::Z'"
//@ diag: 63:40 note "candidate template ignored: could not match 'Z<T>' against 'int'"
//@ diag: 63:40 note "implicit deduction guide declared as 'template <class T> Z(Z<T>) -> Z<T>'"
//@ diag: 63:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 63:44 note "implicit deduction guide declared as 'template <class T> Z(T, int) -> Z<T>'"
//@ diag: 66:71 error "no viable constructor or deduction guide for deduction of template arguments of 'U'"
//@ diag: 66:40 note "candidate template ignored: could not match 'U<T>' against 'int'"
//@ diag: 66:40 note "implicit deduction guide declared as 'template <class T> U(U<T>) -> U<T>'"
//@ diag: 66:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 66:44 note "implicit deduction guide declared as 'template <class T> U(T, int) -> U<T>'"
//@ diag: 68:51 error "no viable constructor or deduction guide for deduction of template arguments of 'U2'"
//@ diag: 67:41 note "candidate template ignored: could not match 'U2<T>' against 'int'"
//@ diag: 67:41 note "implicit deduction guide declared as 'template <class T> U2(U2<T>) -> U2<T>'"
//@ diag: 67:46 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 67:46 note "implicit deduction guide declared as 'template <class T> U2(T, int) -> U2<T>'"
//@ diag: 70:3 error "no viable constructor or deduction guide for deduction of template arguments of 'G'"
//@ diag: 69:26 note "candidate template ignored: could not match 'G<T>' against 'int'"
//@ diag: 69:26 note "implicit deduction guide declared as 'template <class T> G(G<T>) -> G<T>'"
//@ diag: 69:30 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 69:30 note "implicit deduction guide declared as 'template <class T> G(T, int) -> G<T>'"
//@ diag: 73:3 error "no viable constructor or deduction guide for deduction of template arguments of 'Y'"
//@ diag: 71:40 note "candidate template ignored: could not match 'Y<T>' against 'int'"
//@ diag: 71:40 note "implicit deduction guide declared as 'template <class T> Y(Y<T>) -> Y<T>'"
//@ diag: 71:44 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 71:44 note "implicit deduction guide declared as 'template <class T> Y(T, int) -> Y<T>'"
//@ requires: clang<22
// clang 22 emits error: no viable constructor or deduction guide for deduction of template argument...; WP-355 residual
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
