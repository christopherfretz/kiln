// [basic.def.odr]/5, [expr.const]/4: a reference variable whose
// initializer is a constant expression is usable in constant expressions,
// so naming it is not an odr-use, also when it is bound to an array
// element, a subobject of a lifetime-extended temporary, or a temporary
// itself. A lambda without a capture-default may name such a local
// reference (no capture is needed), and the read goes through the
// referee (WP-231). Matches clang 19.
//@ expect: run
//@ stdout: 21 4 9
extern "C" int printf(const char*, ...);
int garr[3] = {1, 2, 3};
struct P { int a, b; };
const P& gp = P{4, 5};
const int& gt = 6;
int main() {
  int& lr = garr[1];
  const int& lb = gp.b;
  auto l = [] { return lr + lb + gt; };
  lr = 10;
  const int& lc = gp.a;
  auto m = [] { return lc; };
  printf("%d %d %d\n", l(), m(), [] { return gt + 3; }());
}
