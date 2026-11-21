// [clang ext] `target` multiversioning on x86-64 (clang AttrDocs
// "target"): a `target("default")` and differing `target` strings make
// one function of several versions, mangled `.avx2`, `.arch_haswell`,
// `.popcnt_sse4.2`. Calls and addresses go to the `ifunc` `f.ifunc`,
// whose resolver (emitted at the end of the module) calls
// `__cpu_indicator_init` and tests `__cpu_model` / `__cpu_features2` in
// order of priority. Internal, inline and member versions; a function
// whose versions are never referenced has no resolver. Compared with
// clang 22's IR exactly (clang 19 gives the resolver no attributes and no
// `dso_local`). spec/codegen.md "Function multiversioning" (WP-1745).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
// Versions in each order, defined and declared.
__attribute__((target("default"))) int f() { return 0; }
__attribute__((target("avx2"))) int f() { return 1; }
__attribute__((target("arch=haswell"))) int f() { return 2; }
__attribute__((target("sse4.2,popcnt"))) int f() { return 3; }
__attribute__((target("arch=skylake"))) int f() { return 4; }
__attribute__((target("arch=cascadelake"))) int f();
int call() { return f(); }
int (*addr())() { return f; }
// A declared default version; the others undefined.
__attribute__((target("avx512f"))) int g(int);
__attribute__((target("default"))) int g(int);
int call_g(int x) { return g(x); }
// Internal and inline versions.
__attribute__((target("default"))) static int s() { return 5; }
__attribute__((target("bmi2"))) static int s() { return 6; }
__attribute__((target("default"))) inline int in() { return 7; }
__attribute__((target("arch=icelake-server"))) inline int in() { return 8; }
int call_s() { return s() + in(); }
// Member functions.
struct S {
  __attribute__((target("default"))) int m() { return 9; }
  __attribute__((target("avx2,fma"))) int m() { return 10; }
};
int call_m(S &x) { return x.m(); }
// Unreferenced versions have no resolver.
__attribute__((target("default"))) int u() { return 11; }
__attribute__((target("avx"))) int u() { return 12; }
