// [basic.lookup.qual.general]/4, [expr.prim.id.dtor]/2: a destructor
// name `~Q` found nowhere. On a class object without a prefix it is
// looked up as in `p->~Q()` (4.3) at the definition; on a dependent
// object in the object's class at instantiation; a dependent qualifier
// that names another class than the object's is "no member named".
// A dependent prefix (`H<T>::In::~Zq`) defers it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:46 error "undeclared identifier 'Zq' in destructor name"
//@ diag: 21:43 error "undeclared identifier 'Zq' in destructor name"
//@ diag: 22:56 error "undeclared identifier 'Zq' in destructor name"
//@ diag: 23:53 error "undeclared identifier 'Zq' in destructor name"
//@ diag: 24:43 error "undeclared identifier 'Zq' in destructor name"
//@ diag: 27:21 note "in instantiation of function template specialization 'a11<S>' requested here"
//@ diag: 25:42 error "no member named '~S' in 'X'"
//@ diag: 27:29 note "in instantiation of function template specialization 'g1<X>' requested here"
struct S { ~S(); };
struct X { ~X(); };
template <class T> struct H { struct In { ~In(); }; };
template <class T> void a5(S *p) { p->H<T>::~Zq(); }
template <class T> void a7(S *p) { p->T::~Zq(); }
template <class T> void a10(S *p) { p->decltype(T())::~Zq(); }
template <class... T> void a12(S *p) { p->T...[0]::~Zq(); }
template <class T> void a11(T &p) { p.T::~Zq(); }
template <class T> void g1(S *s) { s->T::~S(); }
template <class T> void a6(S *p) { p->H<T>::In::~Zq(); }
void f(S s, X *x) { a11(s); g1<X>(&s); }
