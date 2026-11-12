// [stmt.expand], [expr.const]/20, /25.3 (P2996R13): a lambda in a
// run-time `template for` body that names the consteval-only expansion
// variable (no capture: `e` is constexpr and not odr-used) is not an
// error: its call operator is immediate-escalating, so the
// consteval-only expression makes it an immediate function, which is
// then called only in constant expressions. The rule that rejects a
// run-time use (consteval-only-element-neg.cpp) must not reach into it.
// Not GCC-compared: GCC 16.1 rejects a lambda naming a `constexpr auto`
// expansion variable ("use of 'e' before deduction of 'auto'", a GCC
// bug; consteval-only-element-run.cpp compares the `constexpr E e`
// spelling). WP-1330.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
#include <cstdio>
struct E { std::meta::info r; const char* name; };
constexpr E plan[] = {{^^int, "int"}, {^^long, "long"}};
void g() {
  template for (constexpr auto e : plan) {
    auto l = [] { return e.name; };
    constexpr const char* n = l();
    std::puts(n);
  }
}
template <class T>
void t() {
  template for (constexpr auto e : plan) {
    constexpr int len = [] { return e.name[0] == 'i' ? 3 : 4; }();
    std::printf("%d\n", len);
  }
}
int main() { g(), t<int>(); }
