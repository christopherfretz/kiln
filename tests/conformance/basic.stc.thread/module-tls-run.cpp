// [basic.stc.thread]/1-2, [basic.start.dynamic]/7, [basic.start.term]/1:
// each thread has its own instance of an exported or module-linkage
// thread_local variable of a named module; the dynamic initialization of
// the interface unit's thread-local variables runs on a thread's first
// odr-use through a _ZTW wrapper (Itanium ABI §3.4), and their
// destructors run at that thread's exit. Two importers link against the
// one definition. (clang 22 instead re-defines the module's thread-local
// variables in every importer, which fails to link: spec/modules.md §10.)
//@ expect: run
//@ std: c++20
//@ args: -pthread
//@ module: tlm=Inputs/tlm.cppm
//@ unit: Inputs/tlm-user.cpp
//@ stdout: main 1 3 20 103 1
//@ stdout: thread 4 4 50 106 1
//@ stdout: ~T 50
//@ stdout: main 7 28 2
//@ stdout: ~T 20
//@ skip(clang): clang re-defines tlm's thread-local variables in every importer, which then fails to link (spec/modules.md §10 deviation 9)
import tlm;
extern "C" int printf(const char *, ...);
extern "C" int pthread_create(unsigned long *, const void *, void *(*)(void *), void *);
extern "C" int pthread_join(unsigned long, void **);
int second_user();

int local_count() {
  static thread_local int n = 0;
  return ++n;
}

void *body(void *) {
  k += 1;
  int a = x, b = k, c = t.v, d = gety();
  printf("thread %d %d %d %d %d\n", a, b, c, d, local_count());
  return nullptr;
}

int main() {
  int a = x, b = k, c = t.v, d = gety();
  printf("main %d %d %d %d %d\n", a, b, c, d, local_count());
  k = 7;
  unsigned long th;
  pthread_create(&th, nullptr, body, nullptr);
  pthread_join(th, nullptr);
  int s = second_user();
  printf("main %d %d %d\n", k, s, local_count());
}
