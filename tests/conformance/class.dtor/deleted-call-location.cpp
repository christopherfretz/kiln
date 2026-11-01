// [class.dtor]/15, [dcl.fct.def.delete]/2: an explicit destructor call of
// a deleted destructor is ill-formed; clang locates it at the `~` of the
// destructor name (the name's start), and at the destroyed type in an
// instantiated pseudo-destructor call.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:25 error "attempt to use a deleted function"
//@ diag: 14:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 15:33 error "attempt to use a deleted function"
//@ diag: 14:12 note "'~D' has been explicitly marked deleted here"
//@ diag: 16:39 error "attempt to use a deleted function"
//@ diag: 17:15 note "in instantiation of function template specialization 'g<D>' requested here"
//@ diag: 14:12 note "'~D' has been explicitly marked deleted here"
struct D { ~D() = delete; };
void f(D *p, D &r) { p->~D(); r.~D(); }
template <class T> void g(T *p) { p->~T(); }
template void g<D>(D *);
