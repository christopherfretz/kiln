// [temp.inst]/2, [expr.type.conv]/2: `T(a, b)`, `T()` and `T{a, b}` whose
// class template specialization `T` is instantiated there and fails a
// static_assert report only the assertion; the construction of the
// invalid class fails quietly, at that use and later ones (clang's
// `RequireCompleteType` in `BuildCXXTypeConstructExpr`; libc++
// `expected::transform_error` returning `expected<T, unexpected<E>>`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:17 error "static assertion failed due to requirement 'is_ok<int>::value': bad E"
//@ diag: 26:9 note "in instantiation of template class 'E<int>' requested here"
//@ diag: 22:45 error "static assertion failed due to requirement 'is_ok<int>::value': bad F"
//@ diag: 29:9 note "in instantiation of template class 'F<int>' requested here"
//@ diag: 23:45 error "static assertion failed due to requirement 'is_ok<int>::value': bad G"
//@ diag: 30:9 note "in instantiation of template class 'G<int>' requested here"
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
