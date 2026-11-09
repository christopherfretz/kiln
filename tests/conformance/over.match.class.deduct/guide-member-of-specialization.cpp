// [over.match.class.deduct]/1, [temp.deduct]/8, [temp.spec.partial.match]:
// the implicit guide of a constrained constructor names `X::value` of the
// primary template; when the deduced X<int> is an explicit specialization
// (or Y<int *> one instantiated from a partial specialization) the
// constraint names no member of it, so the guide is not viable (clang
// `err_specialization_not_primary_template`; kiln reported "not yet
// implemented"; WP-1071). clang 19 accepts both declarations.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 34:3 error "no viable constructor or deduction guide for deduction of template arguments of 'X'"
//@ diag: 23:27 note "candidate template ignored: could not match 'X<T>' against 'int'"
//@ diag: 23:27 note "implicit deduction guide declared as 'template <class T> X(X<T>) -> X<T>'"
//@ diag: 25:3 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 25:21 note "because substituted constraint expression is ill-formed: cannot reference member of primary template because deduced class template specialization 'X<int>' is an explicit specialization"
//@ diag: 25:3 note "implicit deduction guide declared as 'template <class T> X(T) -> X<T>"
//@ diag: 36:3 error "no viable constructor or deduction guide for deduction of template arguments of 'Y'"
//@ diag: 28:27 note "candidate template ignored: could not match 'Y<T>' against 'int *'"
//@ diag: 28:27 note "implicit deduction guide declared as 'template <class T> Y(Y<T>) -> Y<T>'"
//@ diag: 30:3 note "candidate template ignored: constraints not satisfied [with T = int *]"
//@ diag: 30:21 note "because substituted constraint expression is ill-formed: cannot reference member of primary template because deduced class template specialization 'Y<int *>' is instantiated from a partial specialization"
//@ diag: 30:3 note "implicit deduction guide declared as 'template <class T> Y(T) -> Y<T>"
template <class T> struct X {
  static constexpr int value = 1;
  X(T) requires (X::value > 0) {}
};
template <> struct X<int> { X(int) {} };
template <class T> struct Y {
  static constexpr int value = 1;
  Y(T) requires (Y::value > 0) {}
};
template <class T> struct Y<T *> { Y(T *) {} };
X<long> ok(1L);
X x(1);
int i = 0;
Y y(&i);
