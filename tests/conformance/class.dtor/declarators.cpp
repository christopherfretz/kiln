// [class.dtor]/1-4: a destructor has no return type, parameters, cv-qualifier or
// 'static', names its class, and is a member; [basic.def.odr] redefinition;
// [class.dtor]/4 an implicitly-declared destructor is not defined by the user.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:7 error "destructor cannot have a return type"
//@ diag: 20:3 error "destructor cannot have any parameters"
//@ diag: 21:4 error "undeclared identifier 'T' in destructor name"
//@ diag: 22:8 error "'const' qualifier is not allowed on a destructor"
//@ diag: 23:10 error "destructor cannot be declared 'static'"
//@ diag: 23:10 error "destructor cannot be redeclared"
//@ diag: 19:7 note "previous declaration is here"
//@ diag: 26:3 error "destructor cannot be variadic"
//@ diag: 30:4 error "redefinition of '~V'"
//@ diag: 29:4 note "previous definition is here"
//@ diag: 32:4 error "definition of implicitly declared destructor"
//@ diag: 33:1 error "destructor must be a non-static member function"
struct S {
  int ~S();
  ~S(int);
  ~T();
  ~S() const;
  static ~S();
};
struct U {
  ~U(...);
};
struct V { ~V(); };
V::~V() {}
V::~V() {}
struct W { };
W::~W() {}
~W();
