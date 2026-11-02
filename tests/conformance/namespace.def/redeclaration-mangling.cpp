// [namespace.def.general]/2-3, spec/sema.md §4.6 (WP-325): an entity of a
// namespace inside an inline namespace that is redeclared or defined in
// another definition of that same namespace mangles through it under kiln
// and clang alike (`A::I::R::f` is `_ZN1A1I1R1fEv`). Each asm-label
// declaration below names the clang mangling, so the program links only if
// kiln emits the same symbol. kiln mangles through the first declaration;
// the one kept difference from clang (an entity declared in `A::I::R` and
// first defined in a reopening `namespace A { namespace R {` before any
// use) is outside this common case.
//@ expect: run
//@ strict-diags
extern "C" int printf(const char*, ...);
namespace A {
inline namespace I {
namespace R {
int f();
struct S { static int s(); int g(); };
template <class T> int t(T);
}
}
int use() { return R::f(); }
}
namespace A {
inline namespace I {
namespace R {
int f() { return 1; }
int S::s() { return 2; }
int S::g() { return 3; }
template <class T> int t(T) { return 4; }
template int t(int);
}
}
}
namespace A::inline I::R {
int k();
}
namespace A::inline I::R {
int k() { return 5; }
}
int f_by_name() asm("_ZN1A1I1R1fEv");
int s_by_name() asm("_ZN1A1I1R1S1sEv");
int t_by_name(int) asm("_ZN1A1I1R1tIiEEiT_");
int k_by_name() asm("_ZN1A1I1R1kEv");
int main() {
  A::R::S x;
  printf("%d %d %d %d %d %d\n", f_by_name(), s_by_name(), x.g(), t_by_name(0),
         k_by_name(), A::use());
  return 0;
}
