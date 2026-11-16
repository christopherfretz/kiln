// [basic.scope.scope]/4, [over.dcl]/1: a member function with an explicit
// object parameter corresponds to an implicit object member function with
// the same non-object parameters when the object parameter's type is the
// implicit object parameter's (a non-ref-qualified one also matching
// either reference), so declaring both is a redeclaration error, in a
// class template too. Found running GCC's C++ testsuite.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:19 error "class member cannot be redeclared"
//@ diag: 25:8 note "previous declaration is here"
//@ diag: 26:21 error "class member cannot be redeclared"
//@ diag: 26:8 note "previous declaration is here"
//@ diag: 27:25 error "class member cannot be redeclared"
//@ diag: 27:8 note "previous declaration is here"
//@ diag: 28:31 error "class member cannot be redeclared"
//@ diag: 28:8 note "previous declaration is here"
//@ diag: 29:26 error "static and non-static member functions with the same parameter types cannot be overloaded"
//@ diag: 29:15 note "previous declaration is here"
//@ diag: 30:19 error "class member cannot be redeclared"
//@ diag: 30:8 note "previous declaration is here"
//@ diag: 33:18 error "class member cannot be redeclared"
//@ diag: 33:8 note "previous declaration is here"
struct S {
  void f1(); void f1(this S&&);
  void f2() &; void f2(this S&);
  void f3() const; void f3(this const S&);
  void f4(int) const &&; void f4(this const S&&, int);
  static void f5(); void f5(this S&);
  void f6(); void f6(this S);
};
template<class T> struct X {
  void g(); void g(this X&);
};
X<int> xi;
