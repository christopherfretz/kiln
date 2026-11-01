// [temp.res.general], [dcl.decl]: an error found after substitution is reported at
// the occurrence of the dependent name its declaration wrote, even when the same
// type is written twice in one declaration (spec/sema-templates.md §5.8): a leading
// return type before the parameters, a trailing one after them, a typedef before
// its name, an alias after `=`, a parameter or member from its decl-specifiers.
// Dependent names in expressions (a default argument) carry their own location.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:15 error "no type named 'type' in 'X'"
//@ diag: 44:6 note "in instantiation of template class 'A<X>' requested here"
//@ diag: 33:15 error "no type named 'type' in 'X'"
//@ diag: 34:22 error "no type named 'type' in 'X'"
//@ diag: 35:15 error "no type named 'type' in 'X'"
//@ diag: 36:22 error "no type named 'type' in 'X'"
//@ diag: 37:30 error "no type named 'type' in 'X'"
//@ diag: 38:24 error "no type named 'type' in 'X'"
//@ diag: 39:23 error "no type named 'type' in 'X'"
//@ diag: 39:44 error "no type named 'type' in 'X'"
//@ diag: 40:26 error "no type named 'type' in 'X'"
//@ diag: 40:44 error "no type named 'type' in 'X'"
//@ diag: 41:22 error "no type named 'type' in 'X'"
//@ diag: 41:42 error "no type named 'type' in 'X'"
//@ diag: 42:31 error "no type named 'type' in 'X'"
//@ diag: 46:15 error "no type named 'type' in 'X'"
//@ diag: 49:6 note "in instantiation of template class 'B<X>' requested here"
//@ diag: 46:46 error "no type named 'type' in 'X'"
//@ diag: 47:28 error "no type named 'type' in 'X'"
//@ diag: 50:40 error "no member named 'x' in 'X'"
//@ diag: 51:20 note "in instantiation of default function argument expression for 'dflt<X>' required here"
struct X {};
template <class T> struct A {
  typename T::type f(typename T::type);
  typename T::type g(int, int, int, int, int, typename T::type);
  void h(typename T::type, typename T::type);
  typename T::type v = typename T::type();
  auto t(typename T::type) -> typename T::type;
  auto u(int) -> typename T::type *(*)(typename T::type);
  operator typename T::type();
  typedef typename T::type td; typename T::type x1;
  using al = typename T::type; typename T::type x2;
  static typename T::type s; typename T::type x3;
  void k(int (*p)(typename T::type), typename T::type);
};
A<X> a;
template <class T> struct B {
  typename T::type x0; int (*fp)(typename T::type);
  int (B::*mp)(typename T::type, typename T::type) const;
};
B<X> b;
template <class T> int dflt(int a = T::x) { return a; }
int use() { return dflt<X>(); }
