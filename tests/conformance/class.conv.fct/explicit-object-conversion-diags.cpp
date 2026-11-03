// [class.conv.fct]/1, [dcl.fct]/6 (P0847): a conversion function with an
// explicit object parameter still has no non-object parameters, no cv- or
// ref-qualifier, and is neither static nor virtual; the explicit object
// parameter has no default argument. A constructor or destructor has no
// explicit object parameter.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:3 error "conversion function cannot have any parameters"
//@ diag: 20:25 error "explicit object member function cannot have 'const' qualifier"
//@ diag: 21:26 error "an explicit object parameter cannot appear in a virtual function"
//@ diag: 22:24 error "an explicit object parameter cannot appear in a static function"
//@ diag: 23:29 error "an explicit object parameter can only appear as the first parameter of the function"
//@ diag: 23:3 error "conversion function cannot have any parameters"
//@ diag: 24:26 error "the explicit object parameter cannot have a default argument"
//@ diag: 28:14 error "an explicit object parameter cannot appear in a constructor"
//@ diag: 28:32 error "an explicit object parameter cannot appear in a destructor"
//@ diag: 28:29 error "destructor cannot have any parameters"
struct B {
  operator int(this B, int);
  operator long(this B) const;
  virtual operator short(this B);
  static operator char(this B);
  operator float(int, this B);
  operator double(this B = B());
  operator bool(this B);
};
B::operator bool(this B) { return true; }
struct C { C(this C& self); ~C(this C& self); };
struct O { operator int(this O); };
O::operator int(this O) { return 0; }
