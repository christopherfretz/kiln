// Per-specialization entries (spec/ast.md §18.9.4, WP-2222): a
// using-declarator with a dependent qualifier in a class template is
// redone by each class template specialization ([temp.inst]/3,
// [namespace.udecl]/1), which records what it names there: the member
// it introduces (an inherited constructor for `using T::T;`, one row per
// element of `using Ts::operator()...;`) and the qualifier's dependent
// components (`T::inner::`). A name it introduces, used as a type (`t`
// after `using typename T::t;`), gets the shadow's target with `found`
// the shadow; used in an expression, likewise.
//@ written-refs: inst-using.refs
struct V {
  V() = default;
  V(int) {}
  int size() const { return 1; }
  using t = int;
  struct inner { using u = short; };
};
struct W {
  W() = default;
  W(int) {}
  int size() const { return 2; }
  using t = long;
  struct inner { using u = char; };
};
template <class T> struct D : T, T::inner {
  using T::T;
  using T::size;
  using typename T::t;
  using typename T::inner::u;
  t get() const { return size(); }
  t field = 0;
  u small = 0;
};
template <class T> struct E : D<T> {
  using D<T>::size;
  int more() const { return size(); }
};
struct F1 { void operator()(int) const {} };
struct F2 { void operator()(long) const {} };
template <class... Ts> struct Over : Ts... {
  using Ts::operator()...;
};
int use() {
  D<V> a(1);
  D<W> b(2);
  Over<F1, F2>{}(1);
  return a.get() + b.get() + a.small + E<V>{}.more();
}
