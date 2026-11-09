//@ expect: run
//@ args: -O2
//@ stdout: 3 3 40 s4
//@ stdout: 3 3 70 s7
//@ stdout: 116 6
// [csetjmp.syn]/2 (C 7.13.2.1/3): after longjmp, setjmp returns again and
// the locals of its function keep their values: a volatile one modified
// between the setjmp and the longjmp has the modified value, and one not
// modified since the setjmp has its value. clang marks the library
// setjmp / sigsetjmp `returns_twice` on the declaration and the call (clang
// Sema::AddKnownFunctionAttributes), so the optimizer keeps the frame valid
// across the second return. WP-1073.
#include <csetjmp>
#include <csignal>
#include <cstdio>
#include <setjmp.h>

static std::jmp_buf env;
static sigjmp_buf senv;

__attribute__((noinline)) void jump(int v) { std::longjmp(env, v); }

__attribute__((noinline)) int run(int seed) {
  volatile int count = 0;
  int fixed = seed * 10;
  char buf[16];
  std::snprintf(buf, sizeof buf, "s%d", seed);
  int r = setjmp(env);
  if (count < 3) {
    count = count + 1;
    jump(count);
  }
  std::printf("%d %d %d %s\n", r, count, fixed, buf);
  return count + fixed;
}

__attribute__((noinline)) int run2() {
  volatile int k = 0;
  if (sigsetjmp(senv, 1) < 5) {
    k = k + 2;
    siglongjmp(senv, k);
  }
  return k;
}

int main() {
  int t = run(4);
  t += run(7);
  std::printf("%d %d\n", t, run2());
}
