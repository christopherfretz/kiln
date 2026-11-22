// Per-specialization entries (spec/ast.md §18.9.2, WP-2223): names in
// constraints are resolved by satisfaction ([temp.constr.atomic]/3,
// [temp.constr.sat]). A row is keyed by the declaration the constraint
// belongs to: the concept for its constraint-expression and the
// requirements of a requires-expression in it (`Sized`, `Small`), the
// constrained declaration for its own requires-clause (`len2`), one row
// per distinct resolution, like an alias template's. A requires-expression
// instantiated with a template (`if constexpr (requires { ... })`) is part
// of that instantiation (`body<V>`). A substitution failure records
// nothing (`N`). `len<V>` and `again<V>` satisfy the one atom `Sized<V>`
// (the second is a cache hit): its rows are there once.
//@ written-refs: inst-constraint.refs
struct V { int size() const { return 1; } using value_type = int; };
struct W { int size() const { return 2; } using value_type = long; };
struct N {};
template <class T>
concept Sized = requires(const T &t) {
  t.size();
  typename T::value_type;
};
template <class T>
concept Small = Sized<T> && sizeof(typename T::value_type) <= 4;
template <class T> requires Sized<T> int len(const T &t) { return t.size(); }
template <class T> requires Sized<T> int again(const T &) { return 0; }
template <class T> int len2(const T &t) requires requires { t.size(); } { return 0; }
template <class T> struct Box {
  int get() const requires Sized<T> { return 0; }
};
template <class T> int pick(const T &) { return 0; }
template <class T> requires Small<T> int pick(const T &) { return 1; }
template <class T> int body(const T &t) {
  if constexpr (requires { t.size(); }) return t.size();
  else return 0;
}
int use() {
  V v; W w; N n;
  return len(v) + len(w) + again(v) + len2(v) + Box<V>{}.get() + pick(v) +
         pick(w) + pick(n) + body(v) + body(n) + Sized<V> + Sized<W>;
}
