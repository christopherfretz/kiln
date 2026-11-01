//@ expect: run
//@ strict-diags
//@ oracle: none
//@ args: -Wno-increment-bool
//@ stdout: c 0 1 1
//@ stdout: d 1 0 1
//@ stdout: e 0 1
//@ stdout: if 0 1
//@ stdout: not 1 0
//@ stdout: and 0 1
//@ stdout: or 1 1
//@ stdout: while 1
// WP-183: a post-increment of `_Atomic(bool)` yields the old byte (`i8`,
// as clang). Where it meets an `i1` (a conditional operator's phi, a
// branch, `!`, `&&`, `||`) clang 19 emits invalid IR, so there is no
// oracle; kiln converts it (spec/codegen.md).
extern "C" int printf(const char*, ...);
_Atomic(bool) ab;
bool c(bool x) { return x ? ab++ : false; }
bool d(bool x) { return x ? true : ab++; }
bool e(bool x) { bool r = x ? ab++ : ab++; return r; }
int ifs() { if (ab++) return 1; return 0; }
bool nots() { return !ab++; }
bool ands(bool x) { return x && ab++; }
bool ors(bool x) { return ab++ || x; }
int whiles() { int n = 0; while (!ab++) ++n; return n; }
int main() {
  ab = false; int c1 = c(true); int c2 = c(true); int c3 = c(false);
  printf("c %d %d %d\n", c1, c2, c3 ? 0 : 1);
  ab = false; int d1 = d(true); int d2 = d(false); int d3 = d(false);
  printf("d %d %d %d\n", d1, d2, d3);
  ab = false; int e1 = e(false); int e2 = e(true);
  printf("e %d %d\n", e1, e2);
  ab = false; int i1 = ifs(); int i2 = ifs();
  printf("if %d %d\n", i1, i2);
  ab = false; int n1 = nots(); int n2 = nots();
  printf("not %d %d\n", n1, n2);
  ab = false; int a1 = ands(true); int a2 = ands(true);
  printf("and %d %d\n", a1, a2);
  ab = false; int o1 = ors(true); int o2 = ors(false);
  printf("or %d %d\n", o1, o2);
  ab = false;
  printf("while %d\n", whiles());
}
