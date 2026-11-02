//@ expect: ast
//@ oracle: clang
//@ ast-filter: operator==
//@ requires: clang>=22
// [class.eq]/3: the implicit definition of a defaulted `==` compares the
// bases, then the members (arrays element-wise through an
// ArrayInitLoopExpr-free loop of subscripts), joined with `&&`.
struct B {
  int x;
  bool operator==(const B&) const = default;
};
struct A : B {
  int a;
  int arr[2];
  friend bool operator==(const A&, const A&) = default;
};
bool f(const A& x) { return x == x; }
