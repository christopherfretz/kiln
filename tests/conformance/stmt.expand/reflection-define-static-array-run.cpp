// [stmt.expand] over define_static_array at run time (spec/reflection-
// values.md §4.4-4.5, WP-479): each expansion's constexpr element is the
// frozen value, and the span's data is the one emitted object, so its
// elements are readable through the span in the same expansion.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: run
//@ strict-diags
//@ stdout: 0 10 10
//@ stdout: 1 20 20
//@ stdout: 2 30 30
//@ stdout: ab
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
constexpr int v[] = {10, 20, 30};
constexpr auto sp = std::define_static_array(v);
int main() {
  int i = 0;
  template for (constexpr int x : sp) {
    printf("%d %d %d\n", i, x, sp[i]);
    ++i;
  }
  template for (constexpr char c : std::define_static_array(std::span<const char>("ab", 2))) {
    printf("%c", c);
  }
  printf("\n");
  return 0;
}
