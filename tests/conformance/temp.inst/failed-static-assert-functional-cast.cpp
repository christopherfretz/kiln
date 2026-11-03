// [temp.inst]/2, [expr.type.conv]/2: `T(a, b)`, `T()` and `T{a, b}` whose
// class template specialization `T` is instantiated there and fails a
// static_assert report only the assertion. In a template instantiation
// the construction fails quietly (clang's `RequireCompleteType` in
// `BuildCXXTypeConstructExpr`; libc++ `expected::transform_error`
// returning `expected<T, unexpected<E>>`); outside one the invalid class
// is initialized quietly and its members are still looked up.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:17 error "static assertion failed due to requirement 'is_ok<int>::value': bad E"
//@ diag: 33:9 note "in instantiation of template class 'E<int>' requested here"
//@ diag: 29:45 error "static assertion failed due to requirement 'is_ok<int>::value': bad F"
//@ diag: 36:9 note "in instantiation of template class 'F<int>' requested here"
//@ diag: 30:45 error "static assertion failed due to requirement 'is_ok<int>::value': bad G"
//@ diag: 37:9 note "in instantiation of template class 'G<int>' requested here"
//@ diag: 44:17 error "static assertion failed due to requirement 'is_ok<int>::value': bad X"
//@ diag: 51:50 note "in instantiation of template class 'X<int>' requested here"
//@ diag: 53:27 note "in instantiation of function template specialization 'H<int>::f<int>' requested here"
//@ diag: 56:45 error "static assertion failed due to requirement 'is_ok<int>::value': bad Y"
//@ diag: 57:9 note "in instantiation of template class 'Y<int>' requested here"
//@ diag: 57:18 error "no member named 'm' in 'Y<int>'"
template <class T> struct is_ok { static constexpr bool value = false; };
template <class T> class E {
  static_assert(is_ok<T>::value, "bad E");

public:
  constexpr explicit E(int, int) {}
};
template <class T> struct F { static_assert(is_ok<T>::value, "bad F"); int a, b; };
template <class T> struct G { static_assert(is_ok<T>::value, "bad G"); G(); };

void f() {
  (void)E<int>(1, 2);
  (void)E<int>(1, 2);
  (void)E<int>(1, 2, 3);
  (void)F<int>{1, 2};
  (void)G<int>();
}
// In an instantiation: the class has a base, so it is no aggregate and
// would be constructed by a constructor (none declared: the instantiation
// stopped at the assertion).
template <class T> struct Base { T t; };
template <class T> class X : private Base<T> {
  static_assert(is_ok<T>::value, "bad X");

public:
  template <class... A> constexpr explicit X(int, A &&...) {}
};
template <class T> struct H {
  T v;
  template <class U> constexpr auto f() { return X<U>(1, v); }
};
void g() { (void)H<int>().f<int>(); }
// Outside one, the invalid class is still an object whose members are
// looked up (clang's RecoveryExpr of type `Y<int>`).
template <class T> struct Y { static_assert(is_ok<T>::value, "bad Y"); int m(); };
int u = Y<int>().m();
