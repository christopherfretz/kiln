//@ expect: ast
//@ ast-filter: test
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
// [expr.reflect] P2996R13, spec/reflection-repr.md §5: a reflect-expression
// dumps the operand's kind and entity in a stable form: the type as
// written when it is an alias, the qualified name of a declaration,
// namespace or template, and `::` for the global namespace.
#include "kiln_meta.h"
using namespace std::meta;
namespace ns { int f(); namespace in {} }
namespace na = ns::in;
using I = int;
template <class T> struct Tm {};
struct S { int m; };
enum E { e0 };
consteval void test(int p) {
  info rs[] = {^^int, ^^I, ^^const I *, ^^ns::f, ^^ns, ^^na, ^^::,
               ^^Tm, ^^Tm<int>, ^^S::m, ^^e0, ^^p};
  (void)rs;
}
