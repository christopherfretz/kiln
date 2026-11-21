// [clang ext] `target_clones` on x86-64 (clang AttrDocs "target_clones"):
// one clone per listed version, mangled `.avx2.0`, `.arch_atom.2`,
// `.default.3` (the index in the list), and an `ifunc` with the plain
// name whose resolver tests the clones in order of priority; an `arch=`
// clone keeps the module's `target-cpu`. A duplicate version is emitted
// once. Internal and inline functions, an address, an unreferenced
// function. Compared with clang 22's IR exactly. spec/codegen.md
// "Function multiversioning" (WP-1745).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
__attribute__((target_clones("avx2","sse4.2","arch=atom","default"))) int g(int x) { return x; }
__attribute__((target_clones("avx2","default"))) int h(int);
int call(int x) { return g(x) + h(x); }
__attribute__((target_clones("default","popcnt","avx2","popcnt"))) static int s() { return 1; }
__attribute__((target_clones("arch=x86-64-v2","arch=sandybridge","default"))) inline int in() { return 2; }
int call2() { return s() + in(); }
int (*addr())(int) { return g; }
__attribute__((target_clones("avx512f","default"))) int unref() { return 3; }
