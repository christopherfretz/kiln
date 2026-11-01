// [except.ctor]/2: namespace-scope and static local arrays whose
// destructor may throw are destroyed by `__cxx_global_array_dtor` under a
// partial-array EH cleanup. Exact on x86_64; on aarch64 clang 19 applies
// LoopSimplify/LCSSA to the helper (spec/codegen.md §6, AArch64 loop
// canonicalization), listed as a known deviation in ir_vs_clang.
struct S { int x; S(); ~S() noexcept(false); };
S ga[3];
void f() { static S sa[2]; }
