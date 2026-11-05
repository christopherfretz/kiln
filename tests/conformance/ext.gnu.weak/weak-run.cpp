//@ expect: run
//@ stdout: 1 3 4 5
// GCC "Common Function Attributes"/"Common Variable Attributes", weak: a
// weak declaration that nothing defines resolves to null (clang emits it
// `extern_weak`), and a weak definition gets `weak` linkage (a constant
// too, since clang 15; an inline function keeps its comdat), so it
// still links and is the one used when nothing overrides it. BDE and
// libstdc++ test optional runtime hooks this way (`if (&__pthread_key_create)`,
// bsls's weak `bsls_ident` symbols).
extern "C" int printf(const char *, ...);

extern "C" void kiln_weak_nowhere() __attribute__((weak));
extern "C" int kiln_weak_novar __attribute__((weak));
__attribute__((weak)) int kiln_weak_var = 3;
__attribute__((weak)) extern const int kiln_weak_const = 4;
__attribute__((weak)) int kiln_weak_fn() { return 5; }

int main() {
  bool none = !kiln_weak_nowhere && &kiln_weak_novar == nullptr;
  printf("%d %d %d %d\n", none, kiln_weak_var, kiln_weak_const, kiln_weak_fn());
}
