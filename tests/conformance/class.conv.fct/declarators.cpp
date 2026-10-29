// [class.conv.fct]/1-4: a conversion function is a non-static member with no
// return type, no parameters and not variadic; one to the class itself, a base
// or void is never used (-Wclass-conversion).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:7 error "conversion function cannot have a return type"
//@ diag: 18:3 error "conversion function cannot have any parameters"
//@ diag: 19:3 error "conversion function cannot be variadic"
//@ diag: 20:3 warning "conversion function converting 'A' to itself will never be used"
//@ diag: 21:3 warning "conversion function converting 'A' to itself will never be used"
//@ diag: 22:3 warning "conversion function converting 'A' to 'void' will never be used"
//@ diag: 26:3 warning "conversion function converting 'B' to its base class 'A' will never be used"
//@ diag: 28:1 error "conversion function must be a non-static member function"
//@ diag: 29:10 error "conversion function must be a non-static member function"
struct A {
  operator int();
  int operator long();
  operator short(int);
  operator char(...);
  operator A();
  operator const A &();
  operator void();
  explicit operator bool();
};
struct B : A {
  operator A &();
};
operator int();
explicit operator double();
struct C {
  explicit operator bool() const;
  operator int() const &;
};
C::operator int() const & { return 0; }
