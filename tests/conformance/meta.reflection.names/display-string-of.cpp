// [meta.reflection.names]/6-7 (P2996R13): display_string_of and
// u8display_string_of are implementation-defined; kiln answers GCC
// 16.1's string (spec/reflection.md §3.9): the type as GCC's diagnostics
// print it, fully scoped, and a declaration as GCC's `%D` (a variable by
// its qualified name, a template with its header). Every reflection has
// one, including those with no identifier.
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
static_assert(display_string_of(^^const int*) == "const int*");
static_assert(display_string_of(^^ns::S::mem) == "ns::S::mem");
static_assert(u8display_string_of(^^int) == u8"int");
static_assert(display_string_of(^^ns::S) == "ns::S");
static_assert(display_string_of(^^ns::Tm<int>) == "ns::Tm<int>");
static_assert(display_string_of(^^ns) == "ns");
static_assert(display_string_of(^^ns::Tm) == "template<class T> struct ns::Tm");

int main() {}
