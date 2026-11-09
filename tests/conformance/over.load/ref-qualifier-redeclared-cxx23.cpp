// C++23 [basic.scope.scope]/3: an implicit object member function without a
// ref-qualifier and one with a ref-qualifier have corresponding object
// parameters when their cv-qualifiers agree, so the later declaration
// redeclares a class member, which is ill-formed ([class.mem.general]/5).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 15:8 error "class member cannot be redeclared"
//@ diag: 14:8 note "previous declaration is here"
//@ diag: 17:8 error "class member cannot be redeclared"
//@ diag: 16:8 note "previous declaration is here"
//@ diag: 19:8 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 18:7 note "previous declaration is here"
struct S {
  void f() &;
  void f();
  void g() const;
  void g() const &&;
  int h(int);
  long h(int) &;
};
