// clang `musttail` (WP-809): 10^7-deep mutual recursion at -O0 runs in
// constant stack only if every marked return lowers to `musttail call` +
// `ret`, on both if branches, through a template, a static member, a
// function pointer and an aggregate (sret) return.
//@ expect: run
//@ strict-diags
//@ args: -O0
//@ stdout: 5000000 10000000 10000000 10000000 20000000
struct P { long a, b, c; };
long odd(long n, long acc);
long even(long n, long acc) {
  if (n == 0) { return acc; }
  [[clang::musttail]] return odd(n - 1, acc + 1);
}
long odd(long n, long acc) {
  if (n == 0)
    return acc;
  else
    __attribute__((musttail)) return even(n - 1, acc);
}
template <class T> T down(T n, T acc) {
  if (n == 0)
    return acc;
  [[clang::musttail]] return down<T>(n - 1, acc + 1);
}
struct S {
  static int ping(int n, int acc);
  static int pong(int n, int acc) {
    if (n == 0) return acc;
    [[clang::musttail]] return ping(n - 1, acc + 1);
  }
};
int S::ping(int n, int acc) {
  if (n == 0) return acc;
  [[clang::musttail]] return pong(n - 1, acc + 1);
}
int (*volatile hop)(int, int);
int via_ptr(int n, int acc) {
  if (n == 0) return acc;
  [[clang::musttail]] return hop(n - 1, acc + 1);
}
P agg(long n, long a) {
  if (n == 0) return P{a, a, 2 * a};
  [[clang::musttail]] return agg(n - 1, a + 1);
}
extern "C" int printf(const char *, ...);
int main() {
  hop = via_ptr;
  P p = agg(10000000, 0);
  printf("%ld %ld %d %d %ld\n", even(10000000, 0),
         down<long>(10000000, 0), S::ping(10000000, 0),
         via_ptr(10000000, 0), p.c);
  return 0;
}
