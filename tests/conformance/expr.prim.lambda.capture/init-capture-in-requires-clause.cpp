// [expr.prim.lambda.capture]/6, [basic.scope.lambda] (P2036R3): an
// init-capture is in scope in the lambda's trailing requires-clause,
// with the type deduced from its initializer, in a lambda formed in a
// function template. Found running GCC's C++ testsuite (WP-1472).
//@ expect: compile-ok
//@ std: c++23
//@ xfail(kiln): WP-1497, the requires-clause sees the init-capture before its type is deduced ("atomic constraint that stays value-dependent")
template <class Fn> constexpr auto mk(Fn fn) {
  return [f{fn}](int a) requires requires { f(a); } { return f(a); };
}
static_assert(mk([](int x) { return x + 1; })(1) == 2);
template <class Fn> constexpr auto mk2(Fn fn) {
  return [f{fn}](this auto &&, int a) requires requires { f(a); } { return f(a); };
}
static_assert(mk2([](int x) { return x + 1; })(1) == 2);
