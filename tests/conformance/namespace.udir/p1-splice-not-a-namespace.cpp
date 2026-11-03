// [namespace.udir]/1 P2996R13: the splice-specifier of a using-directive
// shall designate a namespace that is not the global namespace, and shall
// not be dependent.
// kiln is IS-strict on the dependent case; GCC 16.1 accepts a dependent
// `using namespace [: r :];` in a template and resolves it on
// instantiation.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:22 error "splice in a namespace position does not designate a namespace"
//@ diag: 20:22 error "splice in a namespace position does not designate a namespace"
//@ diag: 21:22 error "the splice-specifier of a using-directive cannot designate the global namespace"
//@ diag: 24:22 error "the splice-specifier of a using-directive cannot be dependent"
#include "kiln_meta.h"
namespace N {}
struct S {};
void f() {
  using namespace [: ^^S :];
  using namespace [: ^^int :];
  using namespace [: ^^:: :];
}
template <std::meta::info R> void g() {
  using namespace [: R :];
}
