// [meta.reflection.names]/6-7 (P2996R13): display_string_of and
// u8display_string_of are implementation-defined; kiln answers the
// spelling its diagnostics use: the canonical, fully scoped type for a
// type reflection, the qualified name for a declaration (GCC 16.1
// qualifies too; WP-484). Every reflection has one, including those with
// no identifier.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
using namespace std::meta;

namespace ns {
struct S {
  int mem;
};
template <class T> struct Tm {};
}  // namespace ns

static_assert(display_string_of(^^int) == "int");
static_assert(display_string_of(^^const int*) == "const int *");
static_assert(display_string_of(^^ns::S::mem) == "ns::S::mem");
static_assert(u8display_string_of(^^int) == u8"int");
static_assert(display_string_of(^^ns::S) == "ns::S");
static_assert(display_string_of(^^ns::Tm<int>) == "ns::Tm<int>");
static_assert(display_string_of(^^ns) == "ns");
static_assert(display_string_of(^^ns::Tm) == "ns::Tm");

int main() {}
