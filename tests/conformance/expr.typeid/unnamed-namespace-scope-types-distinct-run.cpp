// [expr.typeid]/2, [basic.link]/4, Itanium ABI 5.1.8: an unnamed class
// and an unnamed enumeration at namespace scope (no typedef name for
// linkage, so no linkage) are distinct types, and so is an unnamed class
// local to a non-inline function: each has its own `type_info`. kiln
// named the namespace-scope ones `Ut_` and gave both one `_ZTIUt_`, so
// they compared equal; they are clang's `$_N` now (spec/landed/abi/WP-1651.md).
//@ expect: run
//@ stdout: 0 0 0 1
#include <cstdio>
#include <typeinfo>

struct { int a; } us;
enum { e1 } ue;
union { int i; } uu;

static const std::type_info &local() {
  struct { int b; } v;
  return typeid(v);
}

int main() {
  std::printf("%d %d %d %d\n", typeid(us) == typeid(ue), typeid(us) == typeid(uu),
              typeid(ue) == local(), typeid(us) == typeid(us));
}
