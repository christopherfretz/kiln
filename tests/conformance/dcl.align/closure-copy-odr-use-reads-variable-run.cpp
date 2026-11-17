// [dcl.align]/2, [expr.prim.lambda.capture]/11, [expr.const]/10: an
// odr-use of a by-copy capture in an alignment-specifier inside a lambda
// body names the closure's member, so by the text `alignas(*&i)` is no
// constant expression; clang 22 and GCC 16.1 read the captured variable and
// accept it (still capturing it), and kiln follows them, as for a non-type
// template argument `T<*&i>` (a recorded deviation,
// docs/status/deviations.md): in a lambda and a generic lambda of a
// non-template function, for `constexpr` and `const` variables, on a
// variable and on a local class. `alignas(i)` is no odr-use and reads the
// variable anyway. GNU `aligned(*&i)` and a lambda in a template keep the
// text's reading (GCC rejects them; closure-copy-odr-use-alignas-template.cpp).
// Found running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
int f() {
  const int i = 16;
  constexpr int k = 32;
  auto g = [=](auto x) {
    alignas(*&i) char c;
    return (int)((unsigned long)&c % 16) + (int)sizeof(x) * 0;
  };
  auto h = [=] {
    alignas(*&i) char a;
    alignas(*&k) char b;
    alignas(i) char c;
    struct alignas(*&k) S { char m; };
    static_assert(alignof(S) == 32, "");
    return (int)((unsigned long)&a % 16 + (unsigned long)&b % 32 + (unsigned long)&c % 16);
  };
  return g(0) + h();
}
int main() { return f(); }
