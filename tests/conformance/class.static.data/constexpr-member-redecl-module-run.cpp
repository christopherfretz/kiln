// [depr.static.constexpr], [class.static.data] (C++17 and later): the
// module interface unit redeclares the `constexpr` member of an exported
// class out of class, so it emits the member's definition (`weak_odr`)
// although it does not use it; the importer refers to that definition
// without emitting one (clang: `available_externally`) and the program
// links (WP-1517; before, the interface unit emitted nothing).
//@ expect: run
//@ std: c++20
//@ module: constexpr_member_redecl=Inputs/constexpr_member_redecl.cppm
import constexpr_member_redecl;

int main() {
  const int *p = &A::n;
  return *p == 3 ? 0 : 1;
}
