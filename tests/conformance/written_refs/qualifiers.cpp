// The written-reference table (spec/ast.md §18), role `nns`: each
// nested-name-specifier component before an expression or member name
// has its own entry, linked by `q=` to the component on its left (`q=::`
// after a leading `::`): namespaces, a namespace alias, classes, a
// class template specialization, a typedef, an enumeration, and a
// template parameter.
//@ written-refs: qualifiers.refs
namespace outer {
namespace inner {
int f() { return 1; }
struct S {
  static int count;
  enum Kind { k0, k1 };
};
template <class T> struct Box {
  static constexpr int size = sizeof(T);
};
} // namespace inner
} // namespace outer
namespace alias = outer::inner;
using SS = outer::inner::S;
int global() { return 2; }

template <class T> int via_param() { return T::count; }

int qualified() {
  return outer::inner::f() + ::outer::inner::f() + alias::f() + ::global() +
         outer::inner::S::count + SS::count + alias::S::k1 + alias::S::Kind::k0 +
         outer::inner::Box<int>::size + via_param<SS>();
}
