// [meta.reflection.member.queries] (P2996R13): members_of a namespace
// lists its members in an unspecified order; kiln gives GCC 16.1's, the
// creation order of each entity's first declaration (gcc/cp/reflect.cc
// members_cmp sorts by DECL_UID; spec/reflection-members.md). An operator
// first declared as a friend inside its class (a downstream record header
// declares operator+/operator- as friends, then redeclares them with the
// comparisons after the class) comes where the friend declaration is. A
// binding generator emits a namespace's free operators in this order.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

namespace ns {
class T {
  friend T operator+(const T&, int);
  friend int operator-(const T&, const T&);
};
bool operator==(const T&, const T&);
T operator+(const T&, int);
T operator+(int, const T&);
int operator-(const T&, const T&);
void after();
}  // namespace ns

consteval bool at(decltype(sizeof 0) i, const char* s) {
  auto ms = members_of(^^ns, access_context::unchecked());
  if (i >= ms.size()) return false;
  std::string_view d = display_string_of(ms[i]);
  decltype(sizeof 0) n = 0;
  for (; s[n]; ++n)
    if (n >= d.size() || d.data()[n] != s[n]) return false;
  return n == d.size();
}
static_assert(members_of(^^ns, access_context::unchecked()).size() == 6);
static_assert(at(0, "ns::T"));
// The friend-declared operator+ and operator- precede operator== though
// their namespace-scope declarations follow it.
static_assert(at(1, "ns::T ns::operator+(const T&, int)"));
static_assert(at(2, "int ns::operator-(const T&, const T&)"));
static_assert(at(3, "bool ns::operator==(const T&, const T&)"));
static_assert(at(4, "ns::T ns::operator+(int, const T&)"));
static_assert(at(5, "void ns::after()"));

int main() {}
