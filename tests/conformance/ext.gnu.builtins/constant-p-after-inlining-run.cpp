// [expr.const] GCC's `__builtin_constant_p` documentation: a call on a
// value that is not a constant expression is left to the optimizer, which
// makes it 1 once inlining turns the argument into a constant; an
// argument with possible side effects, or that is not of arithmetic type,
// is 0 and not evaluated (GCC's execute/bcp-1.c "opt" cases). WP-1288.
//@ expect: run
//@ args: -O2
//@ strict-diags
//@ stdout: 1 1 1 0 0 0 0 | 0 1
extern "C" int printf(const char *, ...);

static inline __attribute__((always_inline)) int is_c(int x) { return __builtin_constant_p(x); }
static inline __attribute__((always_inline)) int is_c1(long long x) {
  return __builtin_constant_p(x + 1);
}
static inline __attribute__((always_inline)) int is_cd(double d) {
  return __builtin_constant_p(d);
}
static inline __attribute__((always_inline)) int is_cp(const char *p) {
  return __builtin_constant_p(p);
}
static inline __attribute__((always_inline)) int is_inc(int x) {
  return __builtin_constant_p(x++);
}

int counter;
int bump() { return ++counter; }
volatile int opaque = 5;

int main() {
  int v = opaque;
  printf("%d %d %d %d %d %d %d | %d %d\n", is_c(1), is_c1(2), is_cd(0.5), is_cp("hi"),
         is_inc(3), __builtin_constant_p(bump()), is_c(v), counter,
         __builtin_constant_p(sizeof(int)));
  return 0;
}
