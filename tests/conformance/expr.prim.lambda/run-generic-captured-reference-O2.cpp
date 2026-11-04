// [expr.prim.lambda.capture]/8, [basic.def.odr]/5: a local reference
// named in a generic lambda's body is implicitly captured when the
// lambda-expression is analysed, before any specialization of its call
// operator exists. The reference might be usable in constant
// expressions, so its capture waits for the end of the full-expression,
// which is reached in the template definition (WP-581: it was never
// captured, and codegen of the specialization reported "reference to an
// unemitted local variable"). Every specialization reaches the referee
// through the closure's member, also as the operand of `&` under a
// `reinterpret_cast`.
// The same program at -O2.
//@ expect: run
//@ std: c++20
//@ args: -O2
//@ stdout: 1 0 1 1 1
extern "C" int printf(const char*, ...);

// The minimal shape: `[&]`, `const auto&`, a reference local.
static bool same(const int& r, int v) {
  const int& rr = r;
  auto f = [&](const auto& x) { return x == rr; };
  return f(v);
}

// `&rr` under a `reinterpret_cast`, two specializations.
static bool through_cast(const int& r) {
  const int& rr = r;
  auto f = [&](const auto& x) {
    return x == *reinterpret_cast<const int*>(&rr) && x == static_cast<int>(*(&rr));
  };
  return f(r) && f(static_cast<long>(r));
}

// A reference whose initializer is not constant, written in a
// non-dependent full-expression of the generic body.
static int nondependent(int v) {
  const int& rv = v;
  return [&](auto x) { int y = rv; return x + y; }(1);
}

int main() {
  int a = 3;
  printf("%d %d %d %d %d\n", same(a, 3), same(a, 4), through_cast(a), nondependent(6) == 7,
         [](int v) { const int& rv = v; return [&](auto x) { return x + rv; }(1) == v + 1; }(7));
}
