// The written-reference table (spec/ast.md §18.5) in declarative
// contexts of a template pattern (stage 3): a name whose lookup waits for
// instantiation has no entity and `dep`, as does a qualifier component
// naming a template parameter's member: a using-declarator of a
// dependent base, a friend through a dependent qualifier, a designator
// of a dependent aggregate. Names that do not depend keep their entity.
// Instantiation records nothing.
//@ written-refs: decl-dependent.refs
struct Plain {
  int size;
  static int get();
};
template <class T> struct Uses : T, Plain {
  using T::get;
  using typename T::value_type;
  using Plain::size;
  friend class T::Builder;
  friend struct Plain;
};
template <class T> struct Shape {
  T width;
};
template <class T> Shape<T> make(T w) { return Shape<T>{.width = w}; }
template <class T> Plain plain() { return Plain{.size = 1}; }
struct Base {
  using value_type = int;
  struct Builder {};
  static int get();
};
Uses<Base> uses;
Shape<int> shape = make(2);
