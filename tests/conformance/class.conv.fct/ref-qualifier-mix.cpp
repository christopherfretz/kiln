// [over.load]/2.3 (C++20): conversion functions and copy assignment
// operators that differ only in a ref-qualifier present on one of them
// cannot be overloaded; a conversion to a dependent type is checked at
// instantiation (WP-1041).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:3 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&&'"
//@ diag: 18:3 note "previous declaration is here"
//@ diag: 21:6 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 20:6 note "previous declaration is here"
//@ diag: 23:3 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 22:3 note "previous declaration is here"
//@ diag: 27:3 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 29:8 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 26:3 note "previous declaration is here"
struct A {
  operator int() &&;
  operator int();
  A &operator=(const A &) &;
  A &operator=(const A &);
  operator long() const &;
  operator long();
};
template<class X> struct C {
  operator X() &;
  operator X() const;
};
C<int> c;
