// [expr.delete]/6, [class.union.general]/3: deleting a union with a
// non-trivial variant member uses its deleted destructor; a private or
// explicitly deleted destructor is diagnosed (also in an instantiation).
// Pinned to C++23 (P3074R7 changes the union rules in C++26: class.union/trivial-union-deleted-cxx26.cpp).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:16 error "attempt to use a deleted function"
//@ diag: 20:13 note "destructor of 'U' is implicitly deleted because variant field 's' has a non-trivial destructor"
//@ diag: 21:26 error "attempt to use a deleted function"
//@ diag: 20:13 note "destructor of 'U' is implicitly deleted because variant field 's' has a non-trivial destructor"
//@ diag: 23:23 error "calling a private destructor of class 'P'"
//@ diag: 22:11 note "implicitly declared private here"
//@ diag: 25:16 error "attempt to use a deleted function"
//@ diag: 24:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 26:34 error "attempt to use a deleted function"
//@ diag: 27:15 note "in instantiation of function template specialization 't<U>' requested here"
//@ diag: 20:13 note "destructor of 'U' is implicitly deleted because variant field 's' has a non-trivial destructor"
struct S { ~S(); };
union U { S s; int i; };
void g(U *p) { delete p; delete[] p; }
class P { ~P(); };
void h(P *p) { delete p; }
struct D { ~D() = delete; };
void k(D *p) { delete p; }
template<class T> void t(T *p) { delete p; }
template void t<U>(U*);
