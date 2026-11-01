// [temp.explicit]/5: an explicit instantiation definition of a function or static
// data member whose template is still undefined at the end of the TU is an error
// (clang PerformPendingInstantiations); a later definition, an explicit
// instantiation declaration, or a class explicit instantiation is not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:23 error "explicit instantiation of undefined member function 'f' of class template 'X<int>'"
//@ diag: 20:8 note "explicit instantiation refers here"
//@ diag: 34:15 error "explicit instantiation of undefined function template 'h'"
//@ diag: 30:24 note "explicit instantiation refers here"
//@ diag: 35:22 error "explicit instantiation of undefined static data member 's' of class template 'X<int>'"
//@ diag: 22:14 note "explicit instantiation refers here"
//@ diag: 36:28 error "explicit instantiation of undefined static data member 'c' of class template 'X<int>'"
//@ diag: 24:20 note "explicit instantiation refers here"
//@ diag: 39:23 error "explicit instantiation of undefined function template 'm'"
//@ diag: 27:26 note "explicit instantiation refers here"
//@ diag: 40:18 error "explicit instantiation of undefined member function 'X' of class template 'X<int>'"
//@ diag: 28:3 note "explicit instantiation refers here"
template<class T> struct X {
  void f();
  void g();
  static int s;
  static int t;
  static const int c = 1;
  static constexpr int k = 2;
  inline static int i = 3;
  template<class U> void m();
  X();
};
template<class T> void h();
template<class T> void later();
template<class T> int vt;
template void X<int>::f();
template void h<int>();
template int X<int>::s;
template const int X<int>::c;
template const int X<int>::k;
template int X<int>::i;
template void X<int>::m<char>();
template X<int>::X();
template void later<int>();
template int X<int>::t;
template int vt<int>;
extern template void X<long>::f();
extern template void h<long>();
extern template int X<long>::s;
template struct X<short>;
template<class T> void X<T>::g() {}
template<class T> void later() {}
template<class T> int X<T>::t = 4;
