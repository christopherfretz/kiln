// [expr.prim.lambda.capture]/6, [basic.scope.lambda] (P2036R3, P2579R0):
// simple captures and init-captures named in a lambda's trailing
// requires-clause, with `mutable`, a trailing return type, `noexcept`,
// an explicit object parameter, and a generic lambda, in lambdas formed
// in function templates (WP-1497).
//@ expect: compile-ok
//@ std: c++23
//@ skip(gcc): GCC 16 rejects init-captures in the requires-clause
template <class Fn> constexpr auto simple(Fn fn) {
  return [fn](int a) requires requires { fn(a); } { return fn(a); };
}
template <class Fn> constexpr auto init(Fn fn) {
  return [f = fn, k = 1](int a) requires requires { f(a + k); } { return f(a + k); };
}
template <class Fn> constexpr auto mut(Fn fn) {
  return [f = fn, n = 0](int a) mutable requires requires { f(a + n); } { return f(a + n++); };
}
template <class Fn> constexpr auto trailing(Fn fn) {
  return [f = fn](int a) -> decltype(f(a)) requires requires { f(a); } { return f(a); };
}
template <class Fn> constexpr auto nothrow(Fn fn) {
  return [f = fn](int a) noexcept requires requires { f(a); } { return f(a); };
}
template <class Fn> constexpr auto self(Fn fn) {
  return [f = fn](this auto &&, int a) requires requires { f(a); } { return f(a); };
}
template <class Fn> constexpr auto generic(Fn fn) {
  return [f = fn](auto a) requires (sizeof(f) > 0) && requires { f(a); } { return f(a); };
}
template <class T> constexpr auto sized(T t) {
  return [u = t](int a) requires (sizeof(u) == sizeof(T)) { return u + a; };
}
constexpr auto inc = [](int x) { return x + 1; };
static_assert(simple(inc)(1) == 2);
static_assert(init(inc)(1) == 3);
static_assert(mut(inc)(1) == 2);
static_assert(trailing(inc)(1) == 2);
static_assert(nothrow(inc)(1) == 2);
static_assert(self(inc)(1) == 2);
static_assert(generic(inc)(1) == 2);
static_assert(sized(1)(1) == 2);
struct NoInt {};
template <class F, class A> concept callable = requires (F f, A a) { f(a); };
static_assert(!callable<decltype(generic([](NoInt) { return 0; })), int>);
