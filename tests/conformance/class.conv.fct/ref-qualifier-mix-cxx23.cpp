// [basic.scope.scope]/3 (C++23): conversion functions and copy assignment
// operators that differ only in a ref-qualifier present on one of them,
// with the same cv-qualifiers, correspond: a redeclaration, which a member
// cannot have (WP-1041).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "conversion function cannot be redeclared"
//@ diag: 13:3 note "previous declaration is here"
//@ diag: 16:6 error "class member cannot be redeclared"
//@ diag: 15:6 note "previous declaration is here"
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
