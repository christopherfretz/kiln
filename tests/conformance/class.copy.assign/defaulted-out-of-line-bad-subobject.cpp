// [class.copy.assign]/12, [dcl.fct.def.default]/5: an assignment
// operator defaulted after its first declaration is defined where it is
// odr-used; a subobject assignment that does not resolve to a usable
// function is an error at that point, reported as the member call
// `t.operator=(other.t)` (clang `buildSingleCopyAssignRecursively`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:49 error "binding reference of type 'Mi' to value of type 'const Mi' drops 'const' qualifier"
//@ diag: 25:31 note "in defaulted copy assignment operator for 'H<Mi>' first required here"
//@ diag: 26:42 note "in instantiation of member function 'H<Mi>::operator=' requested here"
//@ diag: 21:30 note "passing argument to parameter here"
//@ diag: 25:49 error "call to member function 'operator=' is ambiguous"
//@ diag: 25:31 note "in defaulted copy assignment operator for 'H<Am>' first required here"
//@ diag: 26:49 note "in instantiation of member function 'H<Am>::operator=' requested here"
//@ diag: 22:17 note "candidate function"
//@ diag: 22:36 note "candidate function"
//@ diag: 25:49 error "attempt to use a deleted function"
//@ diag: 25:31 note "in defaulted copy assignment operator for 'H<De>' first required here"
//@ diag: 26:56 note "in instantiation of member function 'H<De>::operator=' requested here"
//@ diag: 23:17 note "'operator=' has been explicitly marked deleted here"
struct Mi { Mi& operator=(Mi&); };
struct Am { Am& operator=(Am); Am& operator=(const Am&); };
struct De { De& operator=(const De&) = delete; };
template<class T> struct H { T t; H& operator=(const H&); };
template<class T> H<T>& H<T>::operator=(const H&) = default;
void f(H<Mi>& a, H<Am>& b, H<De>& c) { a = a; b = b; c = c; }
struct B { B& operator=(int); };
struct D : B { using B::operator=; };
struct E { D d; };
void g(E& x, E& y, D& d) { x = y; d = 1; d = d; }
