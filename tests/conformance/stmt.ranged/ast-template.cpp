// [temp.res], [stmt.ranged]: the AST of a range-based for in a template.
// A dependent range keeps only `__range` and the loop variable in the
// pattern; a non-dependent range in the same template is desugared in
// the definition.
//@ expect: ast
//@ ast-filter: sum_dep

template <class C> int sum_dep(C& c) {
  int s = 0;
  for (auto x : c) s += x;
  int a[2] = {};
  for (int x : a) s += x;
  return s;
}
