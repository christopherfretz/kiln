//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: a=3 bb=4
// [meta.reflection.define.aggregate], [stmt.expand] (P2996R13): a run-time
// expansion statement over the members of a defined class reads each
// member through a splice. A reflection is a consteval-only value
// ([basic.types.general]/12), so comparing two of them is done in a
// constexpr variable's initializer, a constant-evaluated context
// ([expr.const]); the same comparison as a plain run-time expression is
// ill-formed (define-run-template-for-neg.cpp). The splice binds a
// reference first: GCC 16.1 rejects `s.[:m:]` of an expansion variable
// written directly as a call argument (consteval-only), but accepts it
// here, and prints the same.
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
using namespace std::meta;

struct S;
consteval {
  define_aggregate(^^S, {data_member_spec(^^int, {.name = "a"}),
                         data_member_spec(^^long, {.name = "bb"})});
}
int main() {
  S s{3, 4};
  constexpr auto ctx = access_context::unchecked();
  template for (constexpr info m : define_static_array(nonstatic_data_members_of(^^S, ctx))) {
    constexpr std::string_view n = identifier_of(m);
    constexpr bool first = m == nonstatic_data_members_of(^^S, ctx)[0];
    auto& v = s.[:m:];
    printf("%s%.*s=%ld", first ? "" : " ", int(n.size()), n.data(), long(v));
  }
  printf("\n");
}
