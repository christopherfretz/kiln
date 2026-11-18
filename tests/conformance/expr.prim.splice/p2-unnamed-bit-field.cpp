// [expr.prim.splice]/2.1: a splice-expression designating an unnamed
// bit-field is ill-formed, in a member access as well as on its own.
// Named bit-fields splice. Found running GCC's C++ testsuite; GCC 16.1
// agrees.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 23:8 error "a splice-expression cannot designate an unnamed bit-field"
//@ diag: 24:12 error "a splice-expression cannot designate an unnamed bit-field"
#include <meta>

struct S {
  int mem;
  int : 5;
  int bits : 3;
};
constexpr auto ctx = std::meta::access_context::unchecked();

void f() {
  S s{};
  s.[: members_of(^^S, ctx)[1] :] = 1;
  (void)[: members_of(^^S, ctx)[1] :];
  s.[: members_of(^^S, ctx)[2] :] = 1;
  s.[: ^^S::bits :] = 2;
}
