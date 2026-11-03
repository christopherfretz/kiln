// [temp.explicit]/12: an explicit instantiation declaration does not
// suppress the implicit instantiation of an inline member function,
// or a constexpr one, so an ill-formed definition of one used is still
// diagnosed; the non-inline static data member's is not instantiated.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:44 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 22:5 note "in instantiation of member function 'X<int>::i' requested here"
//@ diag: 18:52 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 22:18 note "in instantiation of member function 'X<int>::c' requested here"
template <class T> struct X {
  inline void i();
  constexpr int c() const;
  static int s;
};
template <class T> int X<T>::s = T::fine_unused;
template <class T> inline void X<T>::i() { T::error1; }
template <class T> constexpr int X<T>::c() const { T::error2; return 0; }
extern template struct X<int>;
void use(X<int>& x) {
  (void)X<int>::s;
  x.i(); (void)x.c();
}
