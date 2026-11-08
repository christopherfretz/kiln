// [basic.lookup.argdep]/3.1, /4 (P2996R13): std::meta is an associated
// namespace of a std::meta::info argument, so a user function named like
// a std::meta function (here std::meta::rank) meets it in overload
// resolution, and an equally good match is ambiguous. GCC 16.1 rejects
// the same call.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ diag: 15:15 error "call to 'rank' is ambiguous"
#include <meta>

consteval int rank(std::meta::info) { return 0; }
constexpr auto r = ^^int;
static_assert(rank(r) == 0);
static_assert(::rank(r) == 0);
