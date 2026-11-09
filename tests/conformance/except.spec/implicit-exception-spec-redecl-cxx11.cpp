// C++11 [except.spec]/3,14: redeclaring a destructor without a specification
// after an explicit one (and vice versa) is a -Wimplicit-exception-spec-mismatch
// extension warning when the computed implicit specification agrees.  WP-1052.
//@ std: c++11
//@ expect: compile-ok
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 19:4 warning "function previously declared with an explicit exception specification redeclared with an implicit exception specification"
//@ diag: 18:20 note "previous declaration is here"
//@ diag: 21:4 warning "function previously declared with an explicit exception specification redeclared with an implicit exception specification"
//@ diag: 20:12 note "previous declaration is here"
//@ diag: 24:4 warning "function previously declared with an implicit exception specification redeclared with an explicit exception specification"
//@ diag: 23:17 note "previous declaration is here"
//@ diag: 26:5 warning "function previously declared with an explicit exception specification redeclared with an implicit exception specification"
//@ diag: 25:18 note "previous declaration is here"
//@ diag: 28:4 warning "function previously declared with an explicit exception specification redeclared with an implicit exception specification"
//@ diag: 27:12 note "previous declaration is here"
struct X { virtual ~X() noexcept; };
X::~X() {}
struct Y { ~Y() noexcept(true); };
Y::~Y() {}
struct T { ~T() noexcept(false); };
struct Z { T t; ~Z(); };
Z::~Z() noexcept(false) {}
struct Z2 { T t; ~Z2() noexcept(false); };
Z2::~Z2() {}
struct W { ~W() throw(); };
W::~W() {}
