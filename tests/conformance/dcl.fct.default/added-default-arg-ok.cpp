// [dcl.fct.default]/4: a later declaration of a non-template function,
// and an out-of-line member of a non-template class, may add default
// arguments.
//@ std: c++20
void f(int, int);
void f(int, int = 2);
void f(int = 1, int);
struct N { void m(int); N(int, int); template<class U> void g(U, int); };
void N::m(int = 0) {}
N::N(int, int = 0) {}
template<class U> void N::g(U, int) {}
int use() { f(); N n(1); n.m(); return 0; }
