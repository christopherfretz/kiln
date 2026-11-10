//@ expect: ast
//@ oracle: clang
//@ ast-filter: dmiuse
//@ ast-implicit
//@ std: c++23
//@ requires: clang>=22
// [class.temporary]/7 (C++23, P2718R0), [class.temporary]/6 (CWG1815),
// [intro.execution]/5: a default member initializer used in an aggregate
// initialization in a for-range-initializer is rebuilt for the use (its
// `ExprWithCleanups` stripped) and its temporaries are extended by
// `__range1`; one whose reference member a variable's aggregate
// initialization binds keeps its `ExprWithCleanups` and the bound
// temporary is extended by the variable; elsewhere the temporary ends
// with the full-expression (spec/sema.md §7.6a). WP-1177.
struct D {
  int v;
  ~D();
};
struct A {
  int a[2];
  const int* begin() const;
  const int* end() const;
};
struct Agg {
  int x = D{4}.v;
};
struct R {
  const D& r = D{7};
};
template <class T> struct TA {
  T x = D{9}.v;
};
A mk(const Agg&);
A mt(const TA<int>&);
void use(const Agg&);

void dmiuse() {
  for (int x : mk(Agg{}))
    (void)x;
  for (int x : mt(TA<int>{}))
    (void)x;
  R r{};
  use(Agg{});
}
