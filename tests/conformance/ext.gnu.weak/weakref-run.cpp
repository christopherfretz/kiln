//@ expect: run
//@ stdout: undef 1 1
//@ stdout: call 42 8 5
//@ stdout: var 7 9
// GCC "Common Function Attributes", weakref: `weakref("target")` makes the
// declaration a weak reference to `target` (an implicit `alias("target")`);
// the declaration must have internal linkage and emits nothing itself. A
// reference to it is a reference to `target`, weak (it resolves to null
// when nothing defines `target`) unless `target` is also referenced or
// defined normally in the translation unit (clang
// `CodeGenModule::GetWeakRefReference`). libstdc++'s gthr-posix.h declares
// every pthread entry point this way (`__gthrw_(pthread_mutex_lock)`) and
// tests `__gthread_active_p` with the address of one; std::mutex and
// std::thread need it to link.
extern "C" int printf(const char *, ...);

extern "C" int kiln_weakref_nowhere(int);          // never defined
extern "C" int kiln_weakref_add1(int x) { return x + 1; }
extern "C" int kiln_weakref_late(int);              // defined below main

static int nowhere(int) __attribute__((weakref("kiln_weakref_nowhere")));
static __typeof(kiln_weakref_add1) add1 __attribute__((__weakref__("kiln_weakref_add1")));
static int add1b(int) __attribute__((weakref, alias("kiln_weakref_add1")));
static int late(int) __attribute__((weakref("kiln_weakref_late")));

extern "C" int kiln_weakref_novar;                  // never defined
extern "C" int kiln_weakref_var;                    // defined below main
static int novar __attribute__((weakref("kiln_weakref_novar")));
static int var __attribute__((weakref("kiln_weakref_var")));

// gthr-posix.h's __gthread_active_p: the weak address in a static.
static bool active() {
  static void *const p = (void *)&nowhere;
  return p != 0;
}

int main() {
  printf("undef %d %d\n", &nowhere == nullptr, !active());
  printf("call %d %d %d\n", add1(41), add1b(7), late(5));
  var += 2;
  printf("var %d %d\n", &novar == nullptr ? 7 : 0, kiln_weakref_var);
}

extern "C" int kiln_weakref_late(int x) { return x; }
int kiln_weakref_var = 7;
