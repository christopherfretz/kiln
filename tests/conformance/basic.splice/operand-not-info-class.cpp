// [basic.splice]/1: a class operand without a conversion to
// std::meta::info is no splice operand.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 12:13 error "splice operand of type 'const NoConv' is not a reflection"
#include <meta>

struct NoConv {};
constexpr NoConv nc{};
typename [: nc :] x = 1;
