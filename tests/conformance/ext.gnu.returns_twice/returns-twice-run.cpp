//@ expect: run
//@ stdout: 0 1 2
// GCC "Common Function Attributes", returns_twice: a function that may
// return more than once, as `setjmp` (`AttrDocs` "returns_twice");
// a user declaration of `setjmp` carries it.
#include <csetjmp>
#include <cstdio>

extern "C" int setjmp(std::jmp_buf) __attribute__((returns_twice));

static std::jmp_buf env;

int main() {
  volatile int n = 0;
  int r = setjmp(env);
  std::printf(r ? " %d" : "%d", r);
  if (++n < 3)
    std::longjmp(env, n);
  std::printf("\n");
}
