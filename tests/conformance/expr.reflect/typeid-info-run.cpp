// [expr.typeid]/1, [basic.fundamental]/17 P2996R13 (WP-879): typeid of
// std::meta::info names a type_info object for it. The C++ runtime does
// not define that object, so the TU defines it (a weak definition, as
// g++-16 emits `_ZTIDm`); before WP-879 kiln referenced an undefined
// external one and the program failed to link. The name is the info
// type's mangling in the default (kiln) form, `u6__info`; under
// -fkiln-reflection-mangling=gcc it is GCC's `Dm`
// (reflection-mangle-gcc-run.cpp). kiln's spelling differs from GCC's, so
// there is no oracle.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: none
//@ stdout: u6__info 1 1
#include <cstdio>
#include <meta>
#include <typeinfo>

int main() {
  const std::type_info& a = typeid(std::meta::info);
  const std::type_info& b = typeid(decltype(^^int));
  std::printf("%s %d %d\n", a.name(), &a == &b, a == typeid(const std::meta::info));
}
