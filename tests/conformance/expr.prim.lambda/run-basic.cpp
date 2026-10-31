// [expr.prim.lambda]: captureless lambdas called directly, through a
// variable, and through their conversion to pointer to function.
//@ expect: run
//@ stdout: 3 9 42 7 15
extern "C" int printf(const char*, ...);
static int apply(int (*f)(int), int v) { return f(v); }
int main() {
  auto add = [](int a, int b) { return a + b; };
  int (*sq)(int) = [](int x) { return x * x; };
  int r = [] { return 42; }();
  auto st = [](int x) static { return x + 6; };
  printf("%d %d %d %d %d\n", add(1, 2), sq(3), r, st(1), apply([](int x) { return x + 10; }, 5));
}
