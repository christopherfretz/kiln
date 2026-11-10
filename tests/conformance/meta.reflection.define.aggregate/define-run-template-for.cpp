//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(kiln): needs member splices (WP-474), nonstatic_data_members_of (WP-478) and define_static_array (WP-479)
//@ strict-diags
//@ stdout: a=3 bb=4
//@ xfail(gcc): GCC 16.1 rejects the run-time comparison with nonstatic_data_members_of(...)[0] as consteval-only
// [meta.reflection.define.aggregate], [stmt.expand] (P2996R13): a run-time
// expansion statement over the members of a defined class reads each
// member through a splice.
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
    printf("%s%.*s=%ld", m == nonstatic_data_members_of(^^S, ctx)[0] ? "" : " ", int(n.size()),
           n.data(), long(s.[:m:]));
  }
  printf("\n");
}
