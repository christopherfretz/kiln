// [dcl.pre]/12 (P2741R3) with reflection (P2996R13): when the assertion
// fails, its message must be a constant expression; return_type_of on a
// data member throws std::meta::exception, so the message is diagnosed
// once, with the evaluation notes, then the failed assertion. The
// instantiation whose assertion holds (^^S::f) evaluates nothing. GCC
// 16.1 rejects the file too (WP-570; spec/sema.md §4.6).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 26:44 error "the message in a static assertion must be produced by a constant expression"
//@ diag: 29:3 note "in instantiation of function template specialization 'check<^^S::m>' requested here"
//@ diag: 23:58 note "reflection does not represent a function or function type with a return type"
//@ diag: 26:44 note "in call to 'bad_msg(^^S::m)'"
//@ diag: 26:17 error "static assertion failed due to requirement 'std::meta::is_function(...)'"
#include <meta>
#include <string>
#include <string_view>
struct S { int m; int f(); };
consteval std::string_view bad_msg(std::meta::info r) {
  return std::string_view(std::define_static_string(
      std::string("ret ") + std::meta::display_string_of(std::meta::return_type_of(r))));
}
template <std::meta::info R> void check() {
  static_assert(std::meta::is_function(R), bad_msg(R));
}
int main() {
  check<^^S::m>();
  check<^^S::f>();
}
