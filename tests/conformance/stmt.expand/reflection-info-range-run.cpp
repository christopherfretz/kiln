// [stmt.expand] (P1306R5) over a constexpr range of std::meta::info in a
// function that is emitted (P2996R13 [basic.types.general]: info is a
// consteval-only type). Each expansion's R is a constant, so no info
// value reaches run time, and the range object, a variable of
// consteval-only type, is not emitted either (spec/codegen.md
// "Consteval-only variables"; WP-534). Three spellings of the range: a
// variable template holding define_static_array, the call written
// inline, and a static constexpr local array (iterated by the
// destructuring form, whose bindings are checked as the array's
// initializer). GCC 16.1 compiles and runs the same file.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 2 2 2
#include <meta>
#include <cstdio>
namespace ns { struct A { int a; }; struct B { int b; }; }

template <std::meta::info H>
inline constexpr auto entries_v =
    std::define_static_array(std::meta::members_of(H, std::meta::access_context::unchecked()));

template <std::meta::info H>
int count_template() {
  int n = 0;
  template for (constexpr auto R : entries_v<H>) { n += std::meta::identifier_of(R).size(); }
  return n;
}

template <std::meta::info H>
int count_inline() {
  int n = 0;
  template for (constexpr auto R :
                std::define_static_array(std::meta::members_of(H, std::meta::access_context::unchecked()))) {
    n += std::meta::identifier_of(R).size();
  }
  return n;
}

int count_local() {
  int n = 0;
  static constexpr std::meta::info arr[] = {^^ns::A, ^^ns::B};
  template for (constexpr auto R : arr) { n += std::meta::identifier_of(R).size(); }
  return n;
}

int main() {
  std::printf("%d %d %d\n", count_template<^^ns>(), count_inline<^^ns>(), count_local());
  return 0;
}
