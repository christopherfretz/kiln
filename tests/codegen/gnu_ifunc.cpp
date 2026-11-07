// GNU `ifunc("resolver")` (clang `emitIFuncDefinition`; spec/codegen.md
// "Aliases and ifuncs"): external linkage whatever the declaration's,
// a resolver named by its mangled or `extern "C"` name, defined before or
// after the ifunc, gets `disable_sanitizer_instrumentation`; calls name
// the ifunc symbol.
extern "C" int impl1(int x) { return x + 1; }
int impl2(int x) { return x * 2; }
typedef int (*fp)(int);
extern "C" fp resolve() { return impl1; }
extern "C" int ifn(int) __attribute__((ifunc("resolve")));
int ifc(int) __attribute__((ifunc("resolve")));
static int ifs(int) __attribute__((ifunc("resolve")));
int iff(int) __attribute__((ifunc("_ZL4resfv")));
static fp resf() { return impl2; }
extern "C" void *vres() { return reinterpret_cast<void *>(impl2); }
extern "C" int ifv(int) __attribute__((ifunc("vres")));
int use() { return ifn(1) + ifc(2) + ifs(3) + iff(4) + ifv(5); }
fp addr() { return ifc; }
