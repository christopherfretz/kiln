//@ expect: ast
//@ ast-filter: test
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
// [temp.dep.expr] P2996R13, spec/ast-dump.md §5: a dependent
// splice-expression dumps as `CXXSpliceExpr '<dependent type>' lvalue`
// with its operand as the child; a member splice adds ` .` or ` ->` and
// the object expression as its first child.
#include "kiln_meta.h"
struct S { int m; };
namespace test {
template <std::meta::info R> int f(S s) { return [: R :] + s.[: R :]; }
template <std::meta::info R, class T> int g(T* t) { return t->[: R :]; }
}  // namespace test
