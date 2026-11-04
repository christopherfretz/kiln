// [cpp.predefined], [version.syn] (P2996R13, P3491R3): with -freflection
// at C++26, kiln predefines GCC 16.1's __cpp_impl_reflection 202603L, and
// libstdc++ 16.1's <version> and <meta> (through --gcc-toolchain) then
// define __cpp_lib_reflection 202603L and __cpp_lib_define_static 202506L.
// GCC 16.1 compiles the same file.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <version>
#if __cpp_impl_reflection != 202603L
#error "__cpp_impl_reflection"
#endif
#if __cpp_lib_reflection != 202603L || __cpp_lib_define_static != 202506L
#error "<version> reflection macros"
#endif
#include <meta>
static_assert(__cpp_lib_reflection == 202603L);
static_assert(__cpp_lib_define_static == 202506L);
static_assert(std::meta::is_type(^^int));
