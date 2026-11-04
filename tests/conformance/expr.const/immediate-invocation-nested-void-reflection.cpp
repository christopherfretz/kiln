// [expr.const]/16-18 (P2564R3) with reflection: a void consteval call in
// the body of a template for inside a lambda initializing a variable
// template is a nested immediate invocation; the lambda is
// immediate-escalating and the call (whose static_assert checks each
// member reflection) is evaluated in place, as part of the variable
// template's constant initialization (spec/oven.md "Nested immediate
// invocations"; WP-537). GCC 16.1 accepts the same file.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
namespace ns { struct A {}; struct B {}; }
template <std::meta::info H>
inline constexpr auto entries_v =
    std::define_static_array(std::meta::members_of(H, std::meta::access_context::unchecked()));
template <std::meta::info R> consteval void validate() { static_assert(std::meta::is_type(R)); }
template <std::meta::info H>
inline constexpr bool valid_v = [] {
  template for (constexpr auto R : entries_v<H>) validate<R>();
  return true;
}();
static_assert(valid_v<^^ns>);
