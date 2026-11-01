// [expr.call]/14, [conv.lval]: a call returning an lvalue reference is a
// glvalue; using it as a prvalue loads through the returned reference.
//@ expect: run
//@ stdout: 5 6 11 12
extern "C" int printf(const char*, ...);
int &g(int &r) { return r; }
struct S {
  int v;
  int &get() { return v; }
  int &operator[](int) { return v; }
};
int main() {
  int i = 5;
  int r = g(i);
  int s = g(i) + 1;
  S x{11};
  int t = x.get();
  int u = x[0] + 1;
  printf("%d %d %d %d\n", r, s, t, u);
  return 0;
}
