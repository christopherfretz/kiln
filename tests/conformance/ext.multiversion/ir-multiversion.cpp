// [clang ext] Function multiversioning on AArch64 without the `fmv`
// target feature (clang's driver turns it off unless the runtime library
// is compiler-rt): a `target_version("default")` function and a
// `target_clones` function are ordinary functions, and a non-default
// version is never declared. A `target_version` definition is emitted at
// the end of the TU (clang `MayBeEmittedEagerly`), and from clang 22 it
// has the valueless `"fmv-features"` attribute.
// spec/landed/codegen/WP-810.md (WP-810). Host-restricted (no pinned
// `--target`): on an x86_64 host clang has no AArch64 sysroot and leaves
// `+outline-atomics` out of the features.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
// Default versions: declaration and definition in each order.
__attribute__((target_version("default"))) int c14();
int c14() { return 14; }
int c15();
__attribute__((target_version("default"))) int c15() { return 15; }
__attribute__((target_version("default"))) int c16() { return 16; }
int c16();
__attribute__((target_version("default"))) int c17();
__attribute__((__target_version__("default"))) int dd() { return 1; }
// An unreferenced default version is still emitted, after the others;
// a definition before the first `target_version` is emitted in place.
__attribute__((target_version("default"))) int un() { return 7; }
int lt() { return 8; }
__attribute__((target_version("default"))) int lt();
// A non-default version is never declared: `p` and `S::m` are the
// default bodies, and the `sve` bodies are not emitted.
int p();
__attribute__((target_version("sve"))) int p() { return 9; }
struct S { int m(); };
__attribute__((target_version("sve"))) int S::m() { return 1; }
int S::m() { return 2; }
extern "C" __attribute__((target_version("sve"))) int ec() { return 1; }
// target_clones is a plain function without `fmv`.
[[gnu::target_clones("sve", "default")]] int gc() { return 3; }
template <class T> __attribute__((target_version("default"))) T t1(T x) { return x; }
template <class T> __attribute__((target_version("sve"))) T t2(T x) { static_assert(sizeof(T) == 0); return x; }
int use(S &s) { return c14() + c15() + c16() + c17() + dd() + p() + s.m() + gc() + t1(2); }
