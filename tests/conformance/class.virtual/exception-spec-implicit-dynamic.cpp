// C++14 [except.spec]/5, /14-15 (C++11/14): an implicitly declared destructor or
// assignment operator allows exactly the types its implicit callees allow
// (the union of their dynamic lists; any type when one is noexcept(false)),
// and that computed list must be allowed by every overridden function's
// list. Found by the clang negative-test run (WP-912), fixed by WP-957.
// Valid twin: exception-spec-implicit-dynamic-ok.cpp.
//@ std: c++11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 22:21 note "overridden virtual function is here"
//@ diag: 24:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 23:21 note "overridden virtual function is here"
//@ diag: 26:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 22:21 note "overridden virtual function is here"
//@ diag: 28:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 22:21 note "overridden virtual function is here"
//@ diag: 33:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 31:24 note "overridden virtual function is here"
struct A {};
struct B {};
struct X0 { virtual ~X0() throw(A); };
struct X1 { virtual ~X1() throw(B); };
struct X2 : public X0, public X1 {};
struct MB { ~MB() throw(B); };
struct Y1 : X0 { MB m; };
struct MT { ~MT() noexcept(false); };
struct Y3 : X0 { MT m; };
struct D;
struct E {};
struct BA { virtual D &operator=(const D &) throw(A); };
struct MA { MA &operator=(const MA &) throw(E); };
struct D : BA { MA m; };
