// [expr.prim.lambda.capture]/6, [temp.variadic]: an init-capture pack
// named in the lambda's requires-clause or in a nested requirement
// (P2036, P2579). Found running GCC's C++ testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++23
//@ requires: clang>=22
template <class Fn, class... S> constexpr auto mk(Fn fn, S... s) {
  return [fn{fn}, ... s{s}](this auto &&, int a) requires requires { fn(s(a)...); } {
    return fn(s(a)...);
  };
}
constexpr auto add = [](auto... v) { return (v + ... + 0); };
static_assert(mk(add, [](int x) { return x; }, [](int x) { return 2 * x; })(1) == 3);
static_assert(mk(add)(1) == 0);
template <class... S> constexpr auto cnt(S... s) {
  return [... s{s}](auto) requires (sizeof...(s) == 2) { return sizeof...(s); };
}
static_assert(cnt(1, 2)(0) == 2);
