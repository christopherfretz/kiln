// [over.match.class.deduct]/1 (C++23, P2582R1): only a class template that
// inherits constructors gets guides from its base's (D); none when the alias
// template A (C's parameters, defining-type-id B) is not deducible (E, from
// the standard's example); a guide whose return type R does not match B
// (CC<R> has no member type) is not viable (V v2).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ skip(clang): neither clang 19 nor clang 22 implements P2582; the standard's example is the contract
//@ diag: 33:3 error "no viable constructor or deduction guide for deduction of template arguments of 'D'"
//@ diag: 32:30 note "candidate template ignored: could not match 'D<T>' against 'int'"
//@ diag: 32:30 note "implicit deduction guide declared as 'template <typename T> D(D<T>) -> D<T>'"
//@ diag: 32:30 note "candidate template ignored: could not match 'B<T>' against 'int'"
//@ diag: 32:30 note "implicit deduction guide declared as 'template <typename T> D(B<T>) -> D<T>'"
//@ diag: 32:30 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 32:30 note "implicit deduction guide declared as 'template <typename T> D() -> D<T>'"
//@ diag: 35:3 error "no viable constructor or deduction guide for deduction of template arguments of 'E'"
//@ diag: 34:30 note "candidate template ignored: could not match 'E<T>' against 'int'"
//@ diag: 34:30 note "implicit deduction guide declared as 'template <typename T> E(E<T>) -> E<T>'"
//@ diag: 34:30 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 34:30 note "implicit deduction guide declared as 'template <typename T> E() -> E<T>'"
//@ diag: 37:3 error "no viable constructor or deduction guide for deduction of template arguments of 'V'"
//@ diag: 36:27 note "candidate template ignored: could not match 'V<X>' against 'int'"
//@ diag: 36:27 note "implicit deduction guide declared as 'template <class X> V(V<X>) -> V<X>'"
//@ diag: 36:27 note "candidate template ignored: could not match 'X *' against 'int'"
//@ diag: 36:27 note "implicit deduction guide declared as 'template <class X> requires __is_deducible(V, B<X *>) V(X *) -> B<X *>'"
//@ diag: 36:27 note "candidate template ignored: could not match 'B<X *>' against 'int'"
//@ diag: 36:27 note "implicit deduction guide declared as 'template <class X> requires __is_deducible(V, B<X *>) V(B<X *>) -> B<X *>'"
//@ diag: 36:27 note "candidate function template not viable: requires 0 arguments, but 1 was provided"
//@ diag: 36:27 note "implicit deduction guide declared as 'template <class X> V() -> V<X>'"
template <typename T> struct B { B(T); };
template <typename T> struct D : public B<T> {};
D d(42);
template <typename T> struct E : public B<int> { using B<int>::B; };
E e(42);
template <class X> struct V : B<X*> { using B<X*>::B; };
V v2(1);
