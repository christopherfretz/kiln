// [clang ext] Function multiversioning on x86-64, more cases: a lone
// `target("default")` is multiversioned too; a function used before its
// versions are defined, and a version that calls the function (through
// its `ifunc`); the x86-64 microarchitecture levels as `target_clones`
// architectures (`arch=x86-64-v4` ranks above `avx512vl`); `noexcept`
// versions; `cpu_specific` versions defined after their `cpu_dispatch`;
// a `cpu_dispatch` declared twice (clang appends a second, unreachable
// copy of the resolver's body). Compared with clang 22's IR exactly.
// spec/codegen.md "Function multiversioning" (WP-1745).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
// A lone default version is multiversioned too.
__attribute__((target("default"))) int one() { return 1; }
int use_one() { return one(); }
// Used before its versions are defined; a version calls through the ifunc.
__attribute__((target("default"))) int later(int);
__attribute__((target("avx2"))) int later(int);
int use_later(int n) { return later(n); }
__attribute__((target("default"))) int later(int n) { return n ? later(n - 1) : 0; }
__attribute__((target("avx2"))) int later(int n) { return n + 1; }
// x86-64 levels as architectures of clones.
__attribute__((target_clones("arch=x86-64-v4","arch=x86-64-v3","avx512vl","default"))) int lv() { return 3; }
int use_lv() { return lv(); }
// A noexcept multiversioned function.
__attribute__((target("default"))) int ne() noexcept { return 4; }
__attribute__((target("sse4.1"))) int ne() noexcept { return 5; }
int use_ne() noexcept { return ne(); }
// cpu_specific defined after its cpu_dispatch.
__attribute__((cpu_dispatch(atom, generic))) int late();
int use_late() { return late(); }
__attribute__((cpu_specific(atom))) int late() { return 6; }
__attribute__((cpu_specific(generic))) int late() { return 7; }
// A cpu_dispatch declared twice.
__attribute__((cpu_specific(atom))) int twice() { return 8; }
__attribute__((cpu_dispatch(atom, generic))) int twice();
__attribute__((cpu_dispatch(atom, generic))) int twice();
int use_twice() { return twice(); }
