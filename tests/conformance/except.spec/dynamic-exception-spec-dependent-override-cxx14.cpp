// C++14 [except.spec]/3,5: dependent dynamic exception specifications in
// redeclarations compare by dependent type; an overrider's dependent list
// must be a subset of the overridden function's list.  WP-1052.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 27:24 error "exception specification in declaration does not match previous declaration"
//@ diag: 26:24 note "previous declaration is here"
//@ diag: 33:39 error "exception specification of overriding function is more lax than base version"
//@ diag: 32:25 note "overridden virtual function is here"
//@ diag: 33:39 error "exception specification of overriding function is more lax than base version"
//@ diag: 34:9 note "in instantiation of template class 'ns::D<char>' requested here"
//@ diag: 32:25 note "overridden virtual function is here"
//@ diag: 36:41 error "exception specification of overriding function is more lax than base version"
//@ diag: 35:26 note "overridden virtual function is here"
//@ diag: 36:41 error "exception specification of overriding function is more lax than base version"
//@ diag: 37:10 note "in instantiation of template class 'ns::D2<char>' requested here"
//@ diag: 35:26 note "overridden virtual function is here"
struct E1 {};
template<class T> struct D { virtual void k() throw(int); };
template<class T> struct D2 : D<T> { void k() throw(T); };
D2<int> a;
template<class T> void f() throw(T);
template<class T> void f() throw(T);
template<class T> void g() throw(T);
template<class T> void g() throw(T*);
template<class... Ts> void h() throw(Ts...);
template<class... Ts> void h() throw(Ts...);
void u() { h<>(); h<int, E1>(); }
namespace ns {
struct B { virtual void k() throw(int); };
template<class T> struct D : B { void k() throw(T); };
D<char> e;
struct B2 { virtual void k() throw(); };
template<class T> struct D2 : B2 { void k() throw(T); };
D2<char> e2;
}
