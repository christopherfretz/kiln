// [class.abstract]/2: a pure-specifier on a virtual member of a class
// template survives instantiation (WP-141: kiln used to panic setting the
// pure bit after the instantiated FunctionDecl was created).
//@ expect: compile-ok
//@ strict-diags
template <class T> struct A {
  virtual void f() = 0;
  virtual T g() const = 0;
};
template <class T> struct B : A<T> {
  void f() override {}
};
A<int> *p;
static_assert(__is_abstract(A<int>));
static_assert(__is_abstract(B<int>));
static_assert(__is_polymorphic(A<long>));
