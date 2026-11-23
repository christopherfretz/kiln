// [expr.ref]/7, [over.over]/3: `&this->sf<int>` names the specialization
// of a static member function template through an explicit `this`, an
// ordinary function pointer, in a class template and in a class. Kiln
// resolves the name where clang keeps an `UnresolvedMemberExpr` in the
// template: kiln's node kinds need not be clang's, only the written
// extents (spec/ast.md §21), which begin at `this` (the address-of at its
// `&`) and end at the template-id's `>`. `cargo xtask extents` compares
// them with clang (WP-2464).
//@ expect: compile-ok

struct S {
  template <class U> static int sf() { return sizeof(U); }
  static int sg() { return 2; }
};

template <class T> struct X : S {
  auto g() { return &this->sf<T>; }
  auto h() { return &this->sf<int>; }
  auto i() { return &this->sg; }
};

struct Y : S {
  auto g() { return &this->sf<char>; }
};

int use() { return X<short>().g()() + X<short>().h()() + X<short>().i()() + Y().g()(); }
