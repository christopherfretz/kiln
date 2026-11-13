// [dcl.pre]/12 (P2741R3) with reflection (P2996R13, P3491R3): the
// static_assert-message of an assertion that holds is an unevaluated
// operand, so a consteval message builder whose evaluation would fail
// (return_type_of on a data member throws std::meta::exception; a
// thrown int) is never run, in each instantiation (a downstream
// generator shape: a diagnostic string built with
// define_static_string and display_string_of). GCC 16.1 agrees; clang
// evaluates such messages (WP-570; spec/sema.md §4.6).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <string>
#include <string_view>
struct S { int m; int f(); };
consteval std::string_view bad_msg(std::meta::info r) {
  return std::string_view(std::define_static_string(
      std::string("ret ") + std::meta::display_string_of(std::meta::return_type_of(r))));
}
consteval std::string_view bad_msg2() {
  int* p = nullptr;
  if (p == nullptr) throw 1;
  return "never";
}
template <std::meta::info R> void check() {
  static_assert(true, bad_msg(R));
  static_assert(true, bad_msg2());
  static_assert(!std::meta::is_function(R) || std::meta::is_function(R), bad_msg(R));
}
int main() {
  check<^^S::m>();
  check<^^S::f>();
}
