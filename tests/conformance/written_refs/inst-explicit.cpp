// Per-specialization entries (spec/ast.md §18.9.2): an explicit
// instantiation definition ([temp.explicit]/10) instantiates the class
// template specialization and its members (the member functions'
// definitions included, [temp.explicit]/11), and a function template's
// explicit instantiation its definition: each resolves the pattern's
// dependent names as an implicit instantiation does. Names in a member
// function's declaration (its return type) are instantiated with the
// class and keyed by the class template specialization; names in its
// body by the member function of the specialization.
//@ written-refs: inst-explicit.refs
struct Cfg {
  using id = int;
  static id first() { return 7; }
};

template <class T> struct Table {
  typename T::id lookup() const { return T::first(); }
};

template <class T> typename T::id pick() { return T::first(); }

template struct Table<Cfg>;
template Cfg::id pick<Cfg>();
