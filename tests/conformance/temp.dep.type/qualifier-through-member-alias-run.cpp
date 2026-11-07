// [temp.dep.type]/1, [dcl.typedef]/2: inside `RIB<Tr>`, the member alias
// `RegionT` is a synonym for `typename Tr::RegionT`, so the qualified
// names `typename RegionT::PrintStyle` (in the class) and
// `typename Tr::RegionT::PrintStyle` (out of line) denote the same type:
// the out-of-line static data member and member function definitions
// match their declarations. A qualifier naming a dependent type
// canonicalizes to that type's own qualifier and name, as in clang's
// `getCanonicalNestedNameSpecifier`. LLVM's `RegionInfoBase::printStyle`
// (RegionInfo.h, RegionInfoImpl.h). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 1 1 2
#include <cstdio>

template <class F> struct Traits;
template <class Tr> struct RB {
  enum PrintStyle { PrintNone, PrintBB, PrintRN };
};
template <class Tr> struct RIB {
  using RegionT = typename Tr::RegionT;
  using StyleT = typename RegionT::PrintStyle;
  static typename RegionT::PrintStyle printStyle;
  static typename RegionT::PrintStyle get();
  static void set(StyleT s);
};
template <class Tr>
typename Tr::RegionT::PrintStyle RIB<Tr>::printStyle = RB<Tr>::PrintBB;
template <class Tr>
typename Tr::RegionT::PrintStyle RIB<Tr>::get() { return printStyle; }
template <class Tr>
void RIB<Tr>::set(typename Tr::RegionT::PrintStyle s) { printStyle = s; }

template <> struct Traits<int> {
  using RegionT = RB<Traits<int>>;
};

int main() {
  using R = RIB<Traits<int>>;
  int a = R::printStyle;
  int b = R::get();
  R::set(RB<Traits<int>>::PrintRN);
  std::printf("%d %d %d\n", a, b, int(R::get()));
}
