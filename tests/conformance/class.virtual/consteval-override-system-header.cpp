// [class.virtual]/18 (the working draft) forbids a consteval overrider of a
// non-consteval virtual function, but GCC 16.1's libstdc++ `<meta>` declares
// `std::meta::exception::what` as a consteval override of the constexpr
// `std::exception::what` (CWG3117's allowance, which P4101R1 reverted). kiln
// exempts such an overrider when it is declared in a system header, with
// reflection enabled (spec/reflection-repr.md §6); the line marker below
// makes the following lines a system header. User code is rejected
// (consteval-override-consteval-only.cpp).
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
# 1 "meta-shape.h" 1 3
namespace lib {
struct exception {
  constexpr virtual ~exception() = default;
  constexpr virtual const char* what() const noexcept { return "e"; }
};
struct meta_exception : exception {
  decltype(^^int) from;
  consteval const char* what() const noexcept override { return "m"; }
};
}
# 24 "consteval-override-system-header.cpp" 2
consteval bool check() {
  lib::meta_exception e{};
  return e.what()[0] == 'm';
}
static_assert(check());
