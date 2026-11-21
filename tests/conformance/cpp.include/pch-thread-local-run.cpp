// [basic.stc.thread], [basic.start.dynamic]/7, [dcl.stc]/1 (WP-1824):
// a header's thread_local variables (`thread_local`, `__thread`,
// `_Thread_local`, `extern thread_local`, `tls_model`, `constinit`,
// `constexpr`, `const`, an array, a reference, internal, inline, static
// data members, a class with a destructor, a class template's member, a
// variable template, local statics) are initialized on each thread's
// first use and destroyed at its exit. With `--pch-diff`, the header's
// snapshot carries their thread storage duration with the same IR.
//@ expect: run
//@ args: -pthread
//@ stdout: main 190 1 1 1
//@ stdout: thread 190 1 1 0
//@ stdout: ~T 16
//@ stdout: ~T 16
#include "Inputs/pch_thread_local.h"
extern "C" int pthread_create(unsigned long *, const void *, void *(*)(void *), void *);
extern "C" int pthread_join(unsigned long, void **);
thread_local int ptl::ext = ptl::trace(19);
using namespace ptl;
static int *main_d;

int sum() {
  return k + d + ext + st + c11 + ie + ci + cx + cc + arr[2] + ref + internal + inl + S::sm +
         S::si + t.v + Box<int>::v + vt<int>;
}

void *body(void *) {
  int s = sum();
  int b = bump<int>();
  int c = counter();
  printf("thread %d %d %d %d\n", s, b, c, &d == main_d);
  return nullptr;
}

int main() {
  main_d = &d;
  int s = sum();
  int b = bump<int>();
  int c = counter();
  printf("main %d %d %d %d\n", s, b, c, &d == main_d);
  unsigned long th;
  pthread_create(&th, nullptr, body, nullptr);
  pthread_join(th, nullptr);
}
