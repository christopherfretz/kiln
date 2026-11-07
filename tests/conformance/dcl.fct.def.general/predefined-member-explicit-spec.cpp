// [dcl.fct.def.general]/8 (implementation-defined `__PRETTY_FUNCTION__`,
// clang's PredefinedExpr::ComputeName over getTemplateInstantiationPattern),
// [temp.expl.spec]: an explicit specialization of a member function of a
// class template has no instantiation pattern, so its signature prints its
// own parameter types (`m(long)`), where an implicit instantiation prints
// the pattern's (`m(T)`); a member template's explicit specialization
// likewise, and one declared as a member template of a class template
// specialization keeps its own pattern (`C<short>::t(U)`). kiln printed
// `m(T)` for the first. The contract violation site name is this string
// (spec/contracts.md §6.3). Matches clang 19 and 22.
//@ expect: run
//@ stdout: int C<long>::m(long) [T = long]
//@ stdout: int C<int>::m(T) [T = int]
//@ stdout: int C<long>::t(int) [T = long]
//@ stdout: int C<short>::t(U) [T = short, U = int]
#include <cstdio>
template <class T> struct C {
  int m(T) { return std::printf("%s\n", __PRETTY_FUNCTION__); }
  template <class U> int t(U) { return std::printf("%s\n", __PRETTY_FUNCTION__); }
};
template <> int C<long>::m(long) { return std::printf("%s\n", __PRETTY_FUNCTION__); }
template <> template <> int C<long>::t(int) { return std::printf("%s\n", __PRETTY_FUNCTION__); }
template <> template <class U> int C<short>::t(U) {
  return std::printf("%s\n", __PRETTY_FUNCTION__);
}
int main() {
  C<long>{}.m(5L);
  C<int>{}.m(1);
  C<long>{}.t(5);
  C<short>{}.t(5);
}
