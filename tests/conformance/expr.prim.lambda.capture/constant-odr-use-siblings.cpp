// [basic.def.odr]/5 and [expr.prim.lambda.capture]/8: inside a lambda with
// a capture-default, a constant read through the lvalue-to-rvalue
// conversion is not odr-used and not captured; `sizeof(&n)` is unevaluated;
// `&n` and binding a reference to `n` odr-use it, so `[=]` captures it;
// with `[&]` the name still denotes `n` itself ([expr.prim.id.unqual]/3
// adjusts only a capture by copy), whose value is a constant. Found
// running GCC's C++ testsuite.
//@ expect: compile-ok
void g() {
  const int n = 0;
  auto a = [=] { constexpr int i = n; return i; };
  static_assert(sizeof(a) == 1);
  auto b = [&] { constexpr int j = *&n; return j; };
  auto c = [=] { constexpr unsigned long s = sizeof(&n); return s; };
  static_assert(sizeof(c) == 1);
  auto e = [=] { const int &r = n; return r; };
  static_assert(sizeof(e) == sizeof(int));
  auto f = [=] { return &n; };
  static_assert(sizeof(f) == sizeof(int));
  (void)b;
}
