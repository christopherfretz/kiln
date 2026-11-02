// [expr.prim.lambda.capture]/6, [basic.scope.lambda] (P2036R3, a DR so
// also C++20): an init-capture is in scope from the lambda-introducer on:
// in the trailing return type, the noexcept-specifier, the parameter types
// and a lambda template-parameter's default, shadowing an outer variable
// of the same name; in a template it is substituted with the lambda (also
// an init-capture pack), and in a generic lambda's call operator.
//@ expect: run
//@ std: c++20
//@ strict-diags
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

template <class T> constexpr T tmpl(T t) {
  auto l = [y = t, &r = t]() noexcept(sizeof(y) > 1) -> decltype(y) { return y + r; };
  static_assert(same<decltype(l()), T>);
  static_assert(noexcept(l()) == (sizeof(T) > 1));
  return l();
}
static_assert(tmpl(2L) == 4);
static_assert(tmpl('a') == 'a' * 2);

template <class... T> constexpr auto pack(T... t) {
  return [... xs = t]() -> decltype((xs + ...)) { return (xs + ...); }();
}
static_assert(same<decltype(pack(1, 2L)), long>);
static_assert(pack(1, 2L, short(3)) == 6);

auto generic = [z = 1.5](auto a) -> decltype(z + a) { return z + a; };
static_assert(same<decltype(generic(1)), double>);

int main() {
  int y = 0;
  auto l = [y = 2.5](decltype(y) d) noexcept(sizeof(y) == sizeof(double)) -> decltype(y) {
    return y + d;
  };
  static_assert(same<decltype(l(0)), double>);
  static_assert(noexcept(l(0)));
  int k = [y = 3]<class U = decltype(y)>() -> U { return y; }();
  if (l(0.5) != 3.0 || k != 3 || y != 0) return 1;
  if (tmpl(5) != 10 || pack(1, 2) != 3 || generic(2) != 3.5) return 2;
  return 0;
}
