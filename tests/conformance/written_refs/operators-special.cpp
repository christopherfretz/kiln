// Operators whose function is implicit or reached indirectly
// ([class.copy.assign], [class.compare.default], [over.call.object]/2,
// [expr.prim.lambda.closure]; spec/ast.md §18.13.2): an implicitly
// declared or defaulted operator has an entry naming it like any other;
// a call through a surrogate call function names the conversion
// function that yields the pointer to function, at the `(` with the `)`
// as its close; a closure's call operator is a member.
//@ std: c++20
//@ written-refs: operators-special.refs
struct A {
  int v;
  bool operator==(const A &) const = default;
};
using FP = int (*)(int);
struct Callable {
  operator FP() const;
};

void use(A a, A b, Callable c) {
  a = b;
  a == b;
  a != b;
  c(5);
  auto l = [](int x) { return x; };
  l(1);
  auto g = [](auto x) { return x; };
  g(2);
}
