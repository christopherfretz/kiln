// C++17 [except.spec]/4,7-8 (with C++14 [except.spec]/14 type lists): a
// destructor declared without a specification gets the implicit one, the
// union of its callees' dynamic lists, which C++17 still forms from lists
// kept after `-Wdynamic-exception-spec`; a redeclaration compares its written
// specification with it.  WP-1137.
//@ std: c++17
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 29:9 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 29:9 note "use 'noexcept(false)' instead"
//@ diag: 29:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 28:17 note "previous declaration is here"
//@ diag: 30:17 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 30:17 note "use 'noexcept(false)' instead"
//@ diag: 32:9 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 32:9 note "use 'noexcept(false)' instead"
//@ diag: 32:4 warning "function previously declared with an implicit exception specification redeclared with an explicit exception specification"
//@ diag: 31:17 note "previous declaration is here"
//@ diag: 34:4 warning "function previously declared with an implicit exception specification redeclared with an explicit exception specification"
//@ diag: 33:17 note "previous declaration is here"
//@ diag: 36:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 35:17 note "previous declaration is here"
//@ diag: 38:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 37:17 note "previous declaration is here"
struct E1 {};
struct M { ~M() noexcept(false); };
struct A { M m; ~A(); };
A::~A() throw(E1) {}
struct N { ~N() throw(E1); };
struct B { N n; ~B(); };
B::~B() throw(E1) {}
struct C { M m; ~C(); };
C::~C() noexcept(false) {}
struct D { M m; ~D(); };
D::~D() throw() {}
struct F { N n; ~F(); };
F::~F() noexcept(false) {}
