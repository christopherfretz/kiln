// [dcl.constexpr] + [expr.const]: a constexpr function gives the same result
// whether it is evaluated during translation or at run time.
//@ expect: run
//@ stdout: 3628800 3628800 89 89
extern "C" int printf(const char*, ...);

constexpr long factorial(int n) {
  long r = 1;
  for (int i = 2; i <= n; ++i) r *= i;
  return r;
}
constexpr int fib(int n) {
  int a = 0, b = 1;
  while (n-- > 0) {
    int t = a + b;
    a = b;
    b = t;
  }
  return a;
}

constexpr long kFact = factorial(10);
static_assert(kFact == 3628800);
static_assert(fib(11) == 89);

int main(int argc, char**) {
  int n = 9 + argc;
  printf("%ld %ld %d %d\n", kFact, factorial(n), fib(11), fib(n + 1));
}
