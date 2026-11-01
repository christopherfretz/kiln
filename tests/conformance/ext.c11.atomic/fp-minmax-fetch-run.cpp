// WP-165: floating __atomic_{min,max}_fetch and __atomic_fetch_{min,max}.
// clang 19 emits invalid IR for the *_fetch forms (an icmp of floats), so
// there is no oracle; the new value is llvm.minnum / llvm.maxnum of the
// old value and the operand, as atomicrmw fmin / fmax store.
//@ expect: run
//@ oracle: none
//@ stdout: 2 2 5 5
//@ stdout: 2 5 1.5 1.5
//@ stdout: 5 5
extern "C" int printf(const char *, ...);
float f = 5;
double d = 2;
int main() {
  float a = __atomic_min_fetch(&f, 2.0f, __ATOMIC_SEQ_CST);
  float b = f;
  double c = __atomic_max_fetch(&d, 5.0, __ATOMIC_RELAXED);
  double e = d;
  printf("%g %g %g %g\n", a, b, c, e);
  float g = __atomic_fetch_min(&f, 1.5f, __ATOMIC_ACQ_REL);
  double h = __atomic_fetch_max(&d, 1.0, __ATOMIC_SEQ_CST);
  float j = __atomic_min_fetch(&f, 7.0f, 5);
  printf("%g %g %g %g\n", g, h, j, f);
  double i = __atomic_max_fetch(&d, 3.0, 5);
  printf("%g %g\n", i, d);
}
