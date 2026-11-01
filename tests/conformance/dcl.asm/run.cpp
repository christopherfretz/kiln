// [dcl.asm] (GNU extended asm): register and memory operands, tied and
// read-write operands, multiple outputs, and libc++'s DoNotOptimize idiom
// (test_macros.h) run with the same results as clang's build.
//@ expect: run
//@ stdout: 42 7 1
//@ stdout: 10 20
//@ stdout: 5 6
#include <stdio.h>

template <class T> inline void DoNotOptimize(const T &value) { asm volatile("" : : "r,m"(value) : "memory"); }
template <class T> inline void DoNotOptimize(T &value) { asm volatile("" : "+r,m"(value) : : "memory"); }
template <class T> inline void DoNotOptimize_m(T &value) { asm volatile("" : "+m,r"(value) : : "memory"); }

struct P { int a, b; };

int main() {
  int x = 41;
#if defined(__aarch64__)
  asm("add %w0, %w0, #1" : "+r"(x));
#elif defined(__x86_64__)
  asm("addl $1, %0" : "+r"(x));
#else
  x += 1;
#endif
  int y = 7;
  const int cy = y;
  DoNotOptimize(x);
  DoNotOptimize(cy);
  DoNotOptimize_m(y);
  bool b = true;
  asm("" : "+r"(b));
  printf("%d %d %d\n", x, y, b);
  long r, s;
  asm("" : "=r"(r), "=r"(s) : "0"(10L), "1"(20L));
  printf("%ld %ld\n", r, s);
  P p{5, 6};
  asm volatile("" : "+m"(p));
  DoNotOptimize(p);
  printf("%d %d\n", p.a, p.b);
}
