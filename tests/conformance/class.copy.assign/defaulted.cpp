// [class.copy.assign]/7, [dcl.fct.def.default]/2: an explicitly-defaulted
// assignment operator returns X& and has no const qualifier; it is deleted for
// a reference or const member and a subobject without a usable assignment.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:21 error "explicitly-defaulted copy assignment operator must return 'Z &'"
//@ diag: 21:15 error "the parameter for an explicitly-defaulted copy assignment operator must be an lvalue reference type"
//@ diag: 24:7 warning "explicitly defaulted copy assignment operator is implicitly deleted"
//@ diag: 23:8 note "copy assignment operator of 'RF' is implicitly deleted because field 'r' is of reference type 'int &'"
//@ diag: 24:31 note "replace 'default' with 'delete'"
//@ diag: 28:7 warning "explicitly defaulted move assignment operator is implicitly deleted"
//@ diag: 27:13 note "move assignment operator of 'CF' is implicitly deleted because field 'c' is of const-qualified type 'const int'"
//@ diag: 28:26 note "replace 'default' with 'delete'"
//@ diag: 33:7 warning "explicitly defaulted copy assignment operator is implicitly deleted"
//@ diag: 32:6 note "copy assignment operator of 'HM' is implicitly deleted because field 'm' has a deleted copy assignment operator"
//@ diag: 30:17 note "copy assignment operator is implicitly deleted because 'MA' has a user-declared move assignment operator"
//@ diag: 33:31 note "replace 'default' with 'delete'"
//@ diag: 35:17 warning "explicitly defaulted copy assignment operator is implicitly deleted"
//@ diag: 35:17 note "function is implicitly deleted because its declared type does not match the type of an implicit copy assignment operator"
struct Z { const Z &operator=(const Z &) = default; };
struct Y { Y &operator=(Y) = default; };
struct RF {
  int &r;
  RF &operator=(const RF &) = default;
};
struct CF {
  const int c;
  CF &operator=(CF &&) = default;
};
struct MA { MA &operator=(MA &&); };
struct HM {
  MA m;
  HM &operator=(const HM &) = default;
};
struct CQ { CQ &operator=(const CQ &) const = default; };
struct Ok { Ok &operator=(const Ok &) = default; Ok &operator=(Ok &&) = default; int i; };
