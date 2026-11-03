// [expr.prim.id.dtor]/2, [basic.lookup.qual]/5, [basic.life]/1: a
// destructor name deferred because the object type was dependent is
// looked up at instantiation in the object's class only — not found there
// (or a scalar object) it is an undeclared identifier; a scalar
// pseudo-destructor call ends the object's lifetime, so in a constant
// expression the object cannot be destroyed again at scope exit.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:43 note "destroying object 't' whose lifetime has already ended"
//@ diag: 20:15 note "in call to 'ce<int>()'"
//@ diag: 16:40 error "undeclared identifier 'Alias' in destructor name"
//@ diag: 18:19 note "in instantiation of function template specialization 'k8<Z>' requested here"
//@ diag: 16:40 error "undeclared identifier 'Alias' in destructor name"
//@ diag: 18:34 note "in instantiation of function template specialization 'k8<int>' requested here"
template <class T> void k8(T *b) { b->~Alias(); }
struct Z { ~Z(); };
void bad() { Z z; k8(&z); int i; k8(&i); }
template <class T> constexpr int ce() { T t{}; t.~T(); return 1; }
static_assert(ce<int>() == 1);
