// [expr.call]: each call gets fresh automatic objects, so recursion works;
// arguments are copied into the parameters.
//@ expect: run
//@ stdout: 120 6765 3
extern "C" int printf(const char*, ...);

long fact(int n) { return n <= 1 ? 1 : n * fact(n - 1); }
int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int ack(int m, int n) {
  if (m == 0) return n + 1;
  if (n == 0) return ack(m - 1, 1);
  return ack(m - 1, ack(m, n - 1));
}

int main() { printf("%ld %d %d\n", fact(5), fib(20), ack(1, 1)); }
