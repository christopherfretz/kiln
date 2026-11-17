// [meta.reflection.result] reflect_function, reflect_object,
// [expr.const]/10 (P2280R4): an uncaught throw for a constexpr-unknown
// reference. The caught side: reflect-function-unknown-reference.cpp.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 21:20 note "initializer of 'fref' is not a constant expression"
//@ diag: 18:14 note "declared here"
//@ diag: 21:20 error "call to consteval function 'std::meta::reflect_function<int ()>' is not a constant expression"
//@ diag: 21:20 note "initializer of 'fref' is not a constant expression"
//@ diag: 18:14 note "declared here"
#include <meta>
// The line numbers above are absolute.

extern int (&fref)();
// A reference to a function, not usable in constant expressions.

constexpr auto a = std::meta::reflect_function(fref);
