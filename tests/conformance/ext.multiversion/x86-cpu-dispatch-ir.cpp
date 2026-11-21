// [clang ext] `cpu_dispatch` / `cpu_specific` on x86-64 (clang AttrDocs
// "cpu_dispatch"): a `cpu_specific` version per CPU, mangled with the
// CPU's letter (`.S` ivybridge, `.O` atom, `.A` generic); the
// `cpu_dispatch` declaration emits the resolver in place, testing each
// CPU's features in decreasing order of its feature mask, an `ifunc`
// `c.ifunc` and a `weak_odr` alias `c`. A version without a definition
// is declared; without a `generic` CPU the resolver ends in `llvm.trap`.
// Compared with clang 22's IR exactly. spec/codegen.md "Function
// multiversioning" (WP-1745).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
__attribute__((cpu_specific(ivybridge))) int c() { return 1; }
__attribute__((cpu_specific(atom, generic))) int c() { return 2; }
__attribute__((cpu_dispatch(ivybridge, atom, generic))) int c();
int use() { return c(); }
__attribute__((cpu_specific(knl))) void d(int) {}
__attribute__((cpu_dispatch(knl, generic))) void d(int) {}
void use2() { d(1); }
__attribute__((cpu_specific(pentium_iii, core_2_duo_ssse3))) int e();
int use_e() { return e(); }
__attribute__((cpu_dispatch(pentium_iii, core_2_duo_ssse3, pentium, generic))) int e();
// Without a generic CPU the resolver traps.
__attribute__((cpu_specific(haswell))) static int st() { return 3; }
__attribute__((cpu_specific(skylake))) static int st() { return 4; }
__attribute__((cpu_dispatch(haswell, skylake))) static int st();
int use_st() { return st(); }
