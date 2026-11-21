// [basic.stc.thread], [basic.start.dynamic]/7, [dcl.stc]/1,
// spec/modules.md §11.11 "Part 2a" (WP-1824): an importer uses the
// thread_local variables a module exports (`thread_local`, `__thread`,
// `_Thread_local`, `extern thread_local`, `tls_model`, `constinit`,
// `constexpr`, `const`, an array, a reference, inline, static data
// members, a class with a destructor, a local static of an inline
// function), each initialized on its thread's first use through the
// module's `_ZTW` wrappers and `_ZTH` initializers, one object per
// thread across the module and two importers. Under `--bmi-image` the
// module is built from its declaration image, which carries the thread
// storage duration (it fell back to replay before), with the same IR.
// clang 22 importers define the module's thread_local variables again,
// and the program does not link (spec/modules.md §10, deviation 9).
//@ expect: run
//@ std: c++20
//@ args: -pthread
//@ module: imgtl=Inputs/img-thread-local.cppm
//@ unit: Inputs/img-thread-local-user.cpp
//@ skip(clang): clang 22 importers redefine the module's thread_local variables (multiple definitions at the link)
//@ stdout: main 171 171 1 2 1 1
//@ stdout: thread 171 1 1
//@ stdout: ~T 17
//@ stdout: ~T 17
import imgtl;
extern "C" int printf(const char *, ...);
extern "C" int pthread_create(unsigned long *, const void *, void *(*)(void *), void *);
extern "C" int pthread_join(unsigned long, void **);
int user_sum();
int user_counter();
int *user_d();

int sum() {
  return k + d + ext + st + c11 + ie + ci + cx + cc + arr[2] + ref + inl + S::sm + S::si + t.v +
         ns::nested + get_hidden();
}

void *body(void *) {
  int s = sum();
  int c = counter();
  printf("thread %d %d %d\n", s, c, &d == module_d());
  return nullptr;
}

int main() {
  int s = sum();
  int u = user_sum();
  int c1 = counter();
  int c2 = user_counter();
  printf("main %d %d %d %d %d %d\n", s, u, c1, c2, &d == module_d(), user_d() == module_d());
  unsigned long th;
  pthread_create(&th, nullptr, body, nullptr);
  pthread_join(th, nullptr);
}
