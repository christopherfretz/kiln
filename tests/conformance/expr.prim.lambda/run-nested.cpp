// [expr.prim.lambda]: lambdas nested in lambdas, returned as function
// pointers, and stored in arrays.
//@ expect: run
//@ stdout: 11 24 1 4 9
extern "C" int printf(const char*, ...);
using Fn = int (*)(int);
static Fn pick(bool b) {
  if (b) return [](int x) { return x + 1; };
  return [](int x) { return x * 2; };
}
int main() {
  auto outer = [](int x) { return [](int y) { return y + 10; }(x); };
  Fn table[3] = {[](int x) { return x; }, [](int x) { return x * x; }, [](int x) { return x * x; }};
  printf("%d %d %d %d %d\n", outer(1), pick(false)(12), table[0](1), table[1](2), table[2](3));
}
