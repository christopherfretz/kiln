// [namespace.udir]/2: a using-directive at block scope makes the nominated
// namespace's members visible, during unqualified lookup, as if declared in
// the nearest enclosing namespace containing both
//@ expect: compile-ok
//@ strict-diags
namespace lib { int count = 3; namespace inner { int depth = 2; } }
namespace alias = lib::inner;
int use() {
  using namespace lib;
  using namespace alias;
  return count + depth;
}
