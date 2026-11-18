// [meta.reflection.substitute] (P2996R13), the substitute example's fn2,
// [dcl.spec.auto.general]/12: naming fn2<int> is a use of a specialization
// whose return type is deduced when its definition is instantiated, so
// can_substitute(^^fn2, {^^int}) instantiates the body to deduce it; the
// static_assert fails there, outside the immediate context of the
// substitution, and the program is ill-formed (the example says "error").
// GCC 16.1 does not instantiate a namespace-scope function template's body
// for substitute and answers false (it does for member and lambda
// templates; docs/status/deviations.md, WP-1674).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 does not instantiate a namespace-scope function template to deduce its return type in substitute (WP-1674)
//@ strict-diags
//@ diag: 28:19 error "static assertion failed due to requirement '... != ...'"
//@ diag: 32:21 note "in instantiation of function template specialization 'fn2<int>' requested here"
//@ diag: 32:21 note "while substituting template arguments into 'fn2' for std::meta::substitute"
//@ diag: 28:23 note "expression evaluates to '^^int != ^^int'"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;

template<class T>
  auto fn2() {
    static_assert(^^T != ^^int);  // static assertion failed during instantiation of fn2<int>
    return 0;
  }

constexpr bool r2 = can_substitute(^^fn2, {^^int});  // error: instantiation of the body is needed

int main() {}
