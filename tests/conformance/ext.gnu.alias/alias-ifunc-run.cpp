//@ expect: run
//@ stdout: ifunc 42 43 1
//@ stdout: alias 10 11 12 13
//@ stdout: var 6 6 1
// GCC "Common Function Attributes", alias / ifunc: `alias("target")` makes
// the declaration another symbol for the definition named by the mangled
// name `target`; `ifunc("resolver")` makes the declaration an indirect
// function whose address the dynamic loader takes from `resolver()` when
// the object is loaded (clang `EmitAliasDefinition`,
// `emitIFuncDefinition`). Aliases are taken of C++ (mangled), internal
// and `extern "C"` functions and of variables, declared before and after
// their targets. folly's memcpy_select_aarch64.cpp defines
// `__folly_memcpy` this way.
extern "C" int printf(const char *, ...);

extern "C" int kiln_ifunc_wide(int x) { return x + 1; }
extern "C" int kiln_ifunc_narrow(int x) { return x - 1; }
static int add2(int x) { return x + 2; }

// The resolver picks an implementation; it runs before main.
using fn_t = int (*)(int);
extern "C" fn_t kiln_ifunc_resolve() {
  return sizeof(void *) >= 4 ? kiln_ifunc_wide : kiln_ifunc_narrow;
}
extern "C" int kiln_ifunc_pick(int) __attribute__((ifunc("kiln_ifunc_resolve")));
int pick_cxx(int) __attribute__((ifunc("_ZL7resolvev")));     // C++ name
static void *resolve() { return (void *)add2; }               // defined after

int twice(int x) { return 2 * x; }
extern "C" int c_twice(int) __attribute__((alias("_Z5twicei")));
int also_twice(int) __attribute__((alias("_Z5twicei")));
static int s_add2(int) __attribute__((alias("_ZL4add2i")));
extern "C" int c_late(int) __attribute__((alias("kiln_alias_late")));
extern "C" int kiln_alias_late(int x) { return x + 7; }

int counter = 5;
extern int counter_alias __attribute__((alias("counter")));
extern "C" int kiln_alias_cvar __attribute__((alias("counter")));

int main() {
  fn_t p = kiln_ifunc_pick;
  printf("ifunc %d %d %d\n", kiln_ifunc_pick(41), pick_cxx(41), p(0) == 1);
  printf("alias %d %d %d %d\n", c_twice(5), also_twice(5) + 1, s_add2(10),
         c_late(6));
  ++counter_alias;
  printf("var %d %d %d\n", kiln_alias_cvar - 0, counter,
         &counter_alias == &counter);
}
