// [dcl.spec.auto]: deduced variables and return types at run time.
//@ expect: run
//@ stdout: 3 2.5 7 x
extern "C" int printf(const char *, ...);
auto add(int a, int b) { return a + b; }
decltype(auto) pick(int &a, int &b, bool first) {
  if (first)
    return (a);
  return (b);
}
auto half(double d) -> double { return d / 2; }
int main() {
  auto x = add(1, 2);
  auto h = half(5.0);
  int i = 1, j = 2;
  pick(i, j, false) = 7;
  auto c = 'x';
  printf("%d %g %d %c\n", x, h, j, c);
  return 0;
}
