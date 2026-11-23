// [temp.dep.expr]/3, [class.mfct.non.static]/2: in a member function of a
// class template, an unqualified conversion-function-id naming a dependent
// type (`operator T*()`) is the type-dependent implicit member access
// `this->operator T*`. Its written extent (spec/ast.md §21) is the
// written name, `operator` through the conversion-type-id's last token
// (`*`), as clang's `CXXDependentScopeMemberExpr`; the call adds `()`.
// `cargo xtask extents` compares both with clang (WP-2460).
//@ expect: compile-ok

template <class T> struct C {
  T *p;
  operator T *() const { return p; }
  T *g() const { return operator T *(); }
  bool h() const { return operator T *() != nullptr; }
  operator const T &() const { return *p; }
  T k() const { return operator const T &(); }
};

int v = 4;
static_assert(sizeof(C<int>) == sizeof(int *));
int use() {
  C<int> c{&v};
  return *c.g() + int(c.h()) + c.k();
}
