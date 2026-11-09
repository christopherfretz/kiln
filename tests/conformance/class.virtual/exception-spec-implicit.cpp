// [except.spec]/8, [class.copy.assign]/6, [class.dtor]/4: an overrider may not
// be potentially-throwing when the overridden function is not, and that
// holds for implicitly declared members too: an implicit destructor or copy
// or move assignment operator whose computed specification is
// noexcept(false) (a member's throwing destructor or assignment) is checked
// against every overridden function, one error each, as is an explicit
// noexcept(expr) evaluating to false. Found by the clang negative-test run
// (WP-912), fixed by WP-957. Valid twin: exception-spec-implicit-ok.cpp.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 31:15 note "overridden virtual function is here"
//@ diag: 36:21 error "exception specification of overriding function is more lax than base version"
//@ diag: 32:15 note "overridden virtual function is here"
//@ diag: 41:26 error "exception specification of overriding function is more lax than base version"
//@ diag: 38:26 note "overridden virtual function is here"
//@ diag: 41:26 error "exception specification of overriding function is more lax than base version"
//@ diag: 39:26 note "overridden virtual function is here"
//@ diag: 41:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 38:48 note "overridden virtual function is here"
//@ diag: 41:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 39:48 note "overridden virtual function is here"
//@ diag: 43:40 error "exception specification of overriding function is more lax than base version"
//@ diag: 44:10 note "in instantiation of template class 'E<false>' requested here"
//@ diag: 38:26 note "overridden virtual function is here"
struct D1;
struct D2;
struct B {
  B &operator=(const B &);
  virtual D1 &operator=(const D1 &) noexcept;
  virtual D2 &operator=(const D2 &) noexcept;
};
struct M { M &operator=(const M &) noexcept(false); };
struct D1 : B { M m; };
struct D2 : B { D2 &operator=(const D2 &); };

struct A1 { virtual void f() noexcept; virtual ~A1() noexcept; };
struct A2 { virtual void f() noexcept; virtual ~A2() noexcept; };
struct T { ~T() noexcept(false); };
struct C : A1, A2 { void f(); T t; };

template <bool V> struct E : A1 { void f() noexcept(V); };
E<false> e;
