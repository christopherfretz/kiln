// [dcl.fct] [expr.call] [dcl.attr.grammar]/5 Calls through every
// calling convention the host supports agree between caller and callee
// (clang 19 IR conventions; the backend lowers them): `preserve_most`,
// `preserve_all` and `ms_abi` everywhere, `aarch64_vector_pcs` on
// AArch64, `regcall` and `vectorcall` on x86-64, directly, through a
// pointer, virtually and through a lambda conversion, with record
// arguments in registers and in memory. spec/types.md §5.4, WP-808.
//@ expect: run
//@ args: -O1
//@ strict-diags
//@ stdout: pm 21 pa 3.5 ms 60 msr 44 v 7 l 9
//@ stdout: t 42
extern "C" int printf(const char *, ...);
struct Q { int a; double b; };
struct Big { long a, b, c, d, e; };
#define PM __attribute__((preserve_most, noinline))
#define PA __attribute__((preserve_all, noinline))
#define MS __attribute__((ms_abi, noinline))
PM int pm(int a, int b, int c, int d, int e, int f) { return a + b + c + d + e + f; }
PA double pa(double a, float b, Q q) { return a + b + q.b - q.a; }
MS long ms(Q q, Big b, double d) { return q.a + long(q.b) + b.a + b.e + long(d); }
MS Big msr(long x) { return Big{x, x, x, x, x * 40}; }
struct V {
  PM virtual int v(int x) { return x + 1; }
  virtual ~V() = default;
};
struct W : V {
  PM int v(int x) override { return x + 2; }
};
#if defined(__aarch64__)
#define T __attribute__((aarch64_vector_pcs, noinline))
#elif defined(__x86_64__)
#define T __attribute__((regcall, noinline))
#else
#define T __attribute__((noinline))
#endif
T double t1(double a, double b, Q q, Big g) { return a * b + q.a + q.b + double(g.c); }
#if defined(__x86_64__)
__attribute__((vectorcall, noinline)) double t2(double a, double b) { return a - b; }
#else
__attribute__((noinline)) double t2(double a, double b) { return a - b; }
#endif
int main() {
  __attribute__((preserve_most)) int (*p)(int, int, int, int, int, int) = pm;
  volatile int one = 1;
  int r1 = p(one, 2, 3, 4, 5, 6);
  double r2 = pa(1.5, 2.0f, Q{1, 1.0});
  Big b{10, 0, 0, 0, 20};
  long r3 = ms(Q{3, 4.5}, b, 23.0);
  __attribute__((ms_abi)) Big (*mr)(long) = msr;
  Big r4 = mr(one);
  W w;
  V &v = w;
  int r5 = v.v(5);
  auto l = [](int x) __attribute__((preserve_most)) { return x * 3; };
  __attribute__((preserve_most)) int (*lp)(int) = l;
  printf("pm %d pa %.1f ms %ld msr %ld v %d l %d\n", r1, r2, r3, r4.a + r4.e + 3, r5, lp(3));
  double r6 = t1(4.0, 5.0, Q{10, 2.0}, Big{0, 0, 10, 0, 0}) + t2(1.0, 1.0);
  printf("t %d\n", int(r6));
}
