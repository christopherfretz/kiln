// [expr.prim.lambda.closure]/7, [dcl.attr.annotation]/1,
// [meta.reflection.member.queries]: the call operator a closure's
// members_of lists is the one `^^decltype(l)::operator()` names, with the
// annotations of the attribute-specifier-seq before its declarator. A
// closure's function call operator is members-of-eligible
// ([meta.reflection.member.queries]/4, "if Q is a closure type"). Not
// GCC-compared: GCC 16.1's members_of of a closure type does not list
// its call operator (docs/status/deviations.md).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
#include <meta>
using namespace std::meta;

consteval info call_op(info cls) {
  for (info m : members_of(cls, access_context::unchecked()))
    if (is_operator_function(m) && operator_of(m) == op_parentheses)
      return m;
  return info{};
}
auto a = [] [[=1, =2]] () { return 0; };
static_assert(call_op(^^decltype(a)) == ^^decltype(a)::operator());
static_assert(annotations_of(call_op(^^decltype(a))).size() == 2);
