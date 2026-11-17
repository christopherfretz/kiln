// [temp.arg.nontype]/1, [expr.prim.lambda.capture]/11, [expr.const]/10: an
// odr-use of a by-copy capture in a non-type template argument inside a
// lambda body names the closure's member, so by the text `T<*&i>` is no
// constant expression; clang 22 and GCC 16.1 read the captured variable and
// accept it (still capturing it), and kiln follows them, in a lambda, a
// generic lambda and a lambda in a function template (a recorded deviation,
// docs/status/deviations.md), also through a by-reference capture of the
// capture in a lambda called in the argument. Its narrowing check reads the
// same value.
// A case value or a static_assert does not get this reading (WP-1606).
//@ expect: run
//@ strict-diags
template<int N> struct T { static constexpr int v = N; };
int f() {
  const int i = 2;
  auto g = [=](auto x) { T<*&i> t; return t.v + i + (int)sizeof(x); };
  auto h = [=] { return T<*&i>::v + T<[&] { return *&i; }()>::v; };
  if (sizeof h != sizeof(int)) return 100;
  return g(0) + h();
}
template<class Q> int k() {
  const Q j = 3;
  return [=] { return T<*&j>::v; }();
}
int main() { return f() == 2 + 2 + 4 + 2 + 2 && k<int>() == 3 ? 0 : 1; }
