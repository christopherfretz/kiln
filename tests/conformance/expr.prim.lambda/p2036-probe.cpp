// [expr.prim.lambda.capture]/6, [expr.prim.id.unqual]/3 (C++23, P2036R3): an
// init-capture is in scope in the lambda's trailing return type and
// noexcept-specifier. WP-185 feature probe. (clang 19 and 22 still give
// `decltype((x))` of a by-copy capture the outer variable's type in a
// non-mutable lambda, so the probe does not test that part.)
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(kiln): init-capture not in scope in the trailing return type / noexcept
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
void f() {
  auto l = [y = 1L]() noexcept(sizeof(y) > 1) -> decltype(y) { return y; };
  static_assert(same<decltype(l()), long>);
  static_assert(noexcept(l()));
}
