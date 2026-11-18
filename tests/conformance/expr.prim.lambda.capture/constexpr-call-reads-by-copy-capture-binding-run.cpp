// [expr.prim.lambda.capture]/11, [dcl.struct.bind], [expr.const]/10: a
// `constexpr` structured binding (C++26) captured by copy is odr-used
// whenever named ([basic.def.odr]/5), so it names the closure's member; read
// (`a`, also in a generic lambda and with `[a]`) or passed to a call in a
// constant expression in the lambda body, it reads the bound object's
// value, as clang 22 and GCC 16.1 do (GCC rejects `*&a`), while the body
// reads the closure's member at run time (WP-1655, docs/status/deviations.md; the
// other forms are in constexpr-call-reads-by-copy-capture-run.cpp).
//@ requires: clang>=22
//@ expect: run
//@ strict-diags
//@ args: -std=c++26
struct P {
  int v;
  constexpr int get() const { return v; }
};
struct W { P p; };
constexpr int get(const P& p) { return p.v; }

struct A { int x, y; };

int main() {
  constexpr W w{{3}};
  constexpr auto [p] = w;
  constexpr auto [a, b] = A{4, 5};
  auto r1 = [=] { constexpr int j = a + b; return j; };
  auto r2 = [a] { constexpr int j = a; return j; };
  auto r3 = [=](auto) { constexpr int j = a; return j; };
  if (r1() + r2() + r3(0) != 17)
    return 2;
  auto l = [=] {
    constexpr int j = p.get() + get(p);
    return j * 10 + p.get();
  };
  static_assert(sizeof(l) == sizeof(P));
  return l() == 63 ? 0 : 1;
}
