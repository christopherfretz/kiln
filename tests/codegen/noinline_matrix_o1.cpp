//@ args: -O1
// GNU `noinline` / `always_inline` on a definition at -O1 (clang
// `SetLLVMFunctionAttributesForDefinition`): `noinline` alone is kept at
// every level (-O0 adds `optnone` to it, as to every other function);
// `always_inline` gives `alwaysinline`, or `noinline` alone (no
// `optnone`) when `noinline` is present too; the GNU and the standard
// attribute spellings, and either on a redeclaration only, are the same.
// Above -O0 an `inline`-specified function without either (a member
// declared `inline` in its class included) gets `inlinehint`; one that is
// only implicitly inline (`constexpr`, defined in its class) does not.
// An `always_inline` member under `extern template` is inlined and not
// referenced ([temp.explicit]/12; tests/codegen/always_inline.cpp).
__attribute__((noinline)) int ni_gnu(int x) { return x + 1; }
[[gnu::noinline]] int ni_std(int x) { return x + 2; }
[[gnu::noinline]] inline int ni_inline(int x) { return x + 3; }
int ni_redecl(int x);
[[gnu::noinline]] int ni_redecl(int x);
int ni_redecl(int x) { return x + 4; }
__attribute__((always_inline)) inline int ai_gnu(int x) { return x + 5; }
[[gnu::always_inline]] inline int ai_std(int x) { return x + 6; }
__attribute__((always_inline)) int ai_extern(int x) { return x + 7; }
__attribute__((always_inline, noinline)) int both_gnu(int x) { return x + 8; }
[[gnu::always_inline, gnu::noinline]] inline int both_std(int x) { return x + 9; }
[[gnu::always_inline]] int both_redecl(int x);
[[gnu::noinline]] int both_redecl(int x) { return x + 10; }
inline int hinted(int x) { return x + 11; }
constexpr int implicit_inline(int x) { return x + 12; }
struct S {
  int in_class(int x) { return x + 13; }
  inline int declared_inline(int x);
};
int S::declared_inline(int x) { return x + 14; }
template <class T> struct A {
  __attribute__((always_inline)) int f(int x) { return x + 15; }
};
extern template struct A<int>;
int (*keep[])(int) = {ni_inline, ai_gnu, ai_std, both_std, hinted, implicit_inline};
int (S::*keep_m[])(int) = {&S::in_class, &S::declared_inline};
int use(A<int> &a, int x) {
  return ni_gnu(x) + ni_std(x) + ni_inline(x) + ni_redecl(x) + ai_gnu(x) + ai_std(x) +
         ai_extern(x) + both_gnu(x) + both_std(x) + both_redecl(x) + hinted(x) +
         implicit_inline(x) + a.f(x);
}
