// [meta.reflection.substitute] can_substitute: kiln's attempt leaves no
// declaration behind, so a consteval block that would complete a class
// with define_aggregate while can_substitute tries the substitution is
// refused (an error, not `false`); substitute itself may
// (substitute-instantiates-define-aggregate.cpp).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ skip(gcc): GCC 16.1 instantiates and completes the class in can_substitute too
//@ expect: compile-fail
//@ diag: 16:3 error "consteval block is not a constant expression"
//@ diag: 16:15 note "'std::meta::define_aggregate' cannot run while a substitution is being tried (std::meta::substitute)"
//@ diag: 23:15 error "static assertion failed due to requirement 'probe()'"
#include <meta>
template <std::meta::info... Ms> struct Outer {
  struct Inner;
  consteval { std::meta::define_aggregate(^^Inner, {Ms...}); }
};
template <std::meta::info... Ms> using Cls = Outer<Ms...>::Inner;
consteval bool probe() {
  auto m = std::meta::data_member_spec(^^int, {.name = "x"});
  return std::meta::can_substitute(^^Cls, {std::meta::reflect_constant(m)});
}
static_assert(probe());
