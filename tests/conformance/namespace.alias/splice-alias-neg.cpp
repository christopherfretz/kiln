// [namespace.alias]/2: the splice-specifier of a namespace alias shall
// designate a namespace that is not the global namespace. GCC 16.1
// accepts the first and fails on the second; the text decides.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 12:18 error "the splice-specifier of a namespace alias cannot designate the global namespace"
//@ diag: 13:17 error "splice in a namespace position does not designate a namespace"
#include <meta>
struct S {};
namespace G = [: ^^:: :];
namespace T = [:^^S:];
