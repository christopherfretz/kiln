// [class.virtual]/6: a virtual function shall not have a trailing
// requires-clause (clang err_constrained_virtual_method, at the clause's
// first token). Virtual means declared `virtual`, overriding a base's
// virtual function (also found only at instantiation, through a dependent
// base), or redeclaring a virtual function out of class; the check runs
// again in each instantiation of a class template. [temp.mem]/3: a member
// function template is not virtual (clang
// err_virtual_member_function_template), so its clause is not diagnosed.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:29 error "non-templated function cannot have a requires clause"
//@ diag: 27:29 error "virtual function cannot have a requires clause"
//@ diag: 34:21 error "virtual function cannot have a requires clause"
//@ diag: 37:21 error "virtual function cannot have a requires clause"
//@ diag: 37:21 error "virtual function cannot have a requires clause"
//@ diag: 39:9 note "in instantiation of template class 'D2<int>' requested here"
//@ diag: 41:21 error "virtual function cannot have a requires clause"
//@ diag: 43:7 note "in instantiation of template class 'D3<B>' requested here"
//@ diag: 45:29 error "virtual function cannot have a requires clause"
//@ diag: 46:25 error "virtual function cannot have a requires clause"
//@ diag: 49:22 error "'virtual' cannot be specified on member function templates"
//@ diag: 50:3 error "'virtual' cannot be specified on member function templates"
//@ diag: 53:29 error "virtual function cannot have a requires clause"
//@ diag: 55:44 error "virtual function cannot have a requires clause"
struct V {
  virtual void f() requires true && true;
};
struct B {
  virtual void f();
  virtual ~B();
};
template <class T> struct D : B {
  void f() requires true;
};
template <class T> struct D2 : B {
  void f() requires (sizeof(T) > 0);
};
D2<int> d2;
template <class T> struct D3 : T {
  void f() requires true;
};
D3<B> d3;
template <class T> struct E {
  virtual void g() requires true = 0;
  virtual ~E() requires true;
};
template <class T> struct F {
  template <class U> virtual void g() requires true;
  virtual void h(auto) requires true;
};
template <class T> struct Y {
  virtual void f() requires (true || sizeof(T) > 0) && true;
};
template <class T> void Y<T>::f() requires (true || sizeof(T) > 0) && true {}
