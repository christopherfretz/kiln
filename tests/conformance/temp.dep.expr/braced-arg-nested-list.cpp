// [temp.dep.expr]/3, [dcl.init.general]/16.6.2, [over.ics.list]: a
// braced-init-list argument of a parenthesized initializer in a template
// (`T m({{5, 2}, {1, 1}})`, `new T(...)`) is kept as written, nested lists
// included, and list-initializes the constructor's parameter at
// instantiation. libc++'s `flat_map(initializer_list<value_type>, ...)` test
// constructs this way. spec/sema-templates.md "Dependent initializers".
//@ args: -stdlib=libc++
//@ expect: compile-pass
#include <initializer_list>
struct Q {
  int a, b;
  constexpr Q(int x, int y) : a(x), b(y) {}
};
struct M {
  int n = 0;
  constexpr M(std::initializer_list<Q> il) : n(100 + int(il.size())) {
    for (const Q &q : il)
      n += q.a * q.b;
  }
};
template <class T> constexpr int direct() {
  T m({{5, 2}, {1, 1}, {3, 4}});
  return m.n;
}
template <class T> constexpr int mixed() {
  T m({{5, 2}, Q(1, 1)});
  return m.n;
}
template <class T> constexpr int cast() { return T({{5, 2}, {1, 1}}).n; }
template <class T> constexpr int heap() {
  T *p = new T({{5, 2}, {1, 1}});
  int r = p->n;
  delete p;
  return r;
}
static_assert(direct<M>() == 126);
static_assert(mixed<M>() == 113);
static_assert(cast<M>() == 113);
static_assert(heap<M>() == 113);
int main() {}
