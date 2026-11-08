// WP-813: the remaining class B attributes at -O0: likelihood and
// __builtin_expect (nothing at -O0), omp::assume ("llvm.assume" function
// attribute), code_align (`llvm.loop.align` loop metadata),
// loader_uninitialized (an `undef` initializer), randomize_layout (no
// effect without a seed), lto_visibility_public, force_align_arg_pointer
// (x86 only) and assume. spec/landed/codegen/WP-813.md.
void g(); void h();
void f1(int x) { if (x) [[likely]] g(); else h(); }
void f2(int x) { if (__builtin_expect(x, 0)) g(); }
void f3(int x) { switch (x) { [[likely]] case 1: g(); break; case 2: h(); break; } }
void f4(int x) {  g(); }
[[omp::assume("omp_no_openmp")]] void f5() {}
void f6(int *a) { [[clang::code_align(16)]] for (int i = 0; i < 10; ++i) a[i] = 0; }
int lu __attribute__((loader_uninitialized));
struct __attribute__((randomize_layout)) R { int a; char b; int c; };
struct __attribute__((no_randomize_layout)) R2 { int a; };
struct [[clang::lto_visibility_public]] LV { virtual void f(); };
void LV::f() {}
R r;
__attribute__((force_align_arg_pointer)) void f7() {}
[[omp::assume("omp_no_openmp")]] void f8();
[[omp::assume("omp_no_openmp_routines,ompx_foo")]] [[omp::assume("ompx_bar")]] void f9() { f8(); }
void f10(int x) { f9(); [[clang::assume(x > 0)]]; __attribute__((assume(x>1))); }
