// Namespace-scope compound literals in templates ([GNU]; C99 6.5.2.5p5,
// [temp.inst]): a literal in a constexpr variable template, or in a class
// template's default member initializer, is at file scope wherever the
// specialization is instantiated from, so it names one object of static
// storage duration and a constexpr pointer to it is a constant expression
// (as clang 19 and GCC). Deviation (docs/status/deviations.md, WP-490):
// clang 21+ takes the parser's scope at the point of instantiation
// (llvm-project 977d8a4bcd83, `Scope::isInCFunctionScope`), so
// instantiated from a function body the literal is a block-scope
// temporary and clang 22 rejects `p<int>` and `S<int>::q`.
//@ expect: run
//@ exit: 0
//@ skip(clang): clang 22 makes the literal a temporary when instantiated from a function (the deviation)
template <class T> constexpr const T *p = (const T[]){1, 2, 3};
template <class T> constexpr T v = ((const T[]){1, 2, 3})[1];
template <class T> struct S { const T *q = (const T[]){4, 5}; };

const int *first() { return p<int>; }
int sum() {
  static_assert(p<int>[2] == 3 && v<int> == 2);
  return p<int>[0] + v<int>;
}
int member() {
  S<int> s;
  return s.q[1];
}
const long *h = p<long>;

int main() {
  if (sum() != 3 || member() != 5)
    return 1;
  // One object: every use names the same literal.
  if (first() != p<int> || first() + 2 != &p<int>[2] || h[1] != 2)
    return 2;
  S<int> a, b;
  if (a.q != b.q)
    return 3;
  return 0;
}
