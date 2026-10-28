// Direct, recursive, static and indirect calls.
static int sq(int x) { return x * x; }
int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
typedef int (*fn)(int);
fn table[2] = {sq, fib};
int apply(int i, int v) { return table[i](v) + sq(v); }
