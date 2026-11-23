// [namespace.udecl]/1, [temp.dep.type]/6: a member named by a dependent
// using-declarator (`using T::f;` then `f()`) is an implicit
// `this->f` looked up at instantiation. The qualifier is the
// declarator's, not written at the use, so its written extent (spec/ast.md
// §21) begins at the written name, as clang's `UnresolvedMemberExpr`;
// with a written qualifier naming the class itself (`U::v`), at that
// qualifier. `cargo xtask extents` compares them with clang (WP-2461).
//@ expect: compile-ok

struct B {
  int f() { return 1; }
  int f(int x) { return x; }
  int v = 3;
  static int s() { return 5; }
};

template <class T> struct U : T {
  using T::f;
  using T::v;
  using T::s;
  int h() { return f() + f(2) + v + U::v + s(); }
};

int use() { return U<B>().h(); }
