// A diagnostic's type argument desugars an alias of a vector typedef down to
// the vector's typedef, which it keeps as written with its scope (clang's
// `Desugar` stops before the vector; [dcl.typedef]/2): 'B' (aka 'X::v'),
// and in an instantiation 'A<long>' (aka 't1<long>::v'), the `typename`
// keyword of the dependent name gone. WP-187.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:30 error "cannot initialize a variable of type 'int *' with an lvalue of type 'B' (aka 'X::v')"
//@ diag: 20:42 error "cannot initialize a variable of type 'int *' with an lvalue of type 'C' (aka 'int')"
//@ diag: 20:54 error "cannot initialize a variable of type 'int *' with an lvalue of type 'D' (aka 'N::u')"
//@ diag: 24:53 error "cannot initialize a variable of type 'int *' with an lvalue of type 'A<long>' (aka 't1<long>::v')"
//@ diag: 25:15 note "in instantiation of function template specialization 'k<long>' requested here"
//@ diag: 24:65 error "cannot initialize a variable of type 'int *' with an lvalue of type 'A2<long>' (aka 'long')"
//@ diag: 26:89 error "cannot initialize a variable of type 'int *' with an lvalue of type 'E' (aka 't1<short>::v')"
//@ diag: 27:17 note "in instantiation of member function 't2<short>::g' requested here"
//@ diag: 26:101 error "cannot initialize a variable of type 'int *' with an lvalue of type 'F' (aka 'short')"
struct X { typedef int v __attribute__((vector_size(16))); typedef int w; };
namespace N { typedef int u __attribute__((vector_size(16))); }
using B = X::v; using C = X::w; using D = N::u;
void f(B b, C c, D d) { int *p = b; int *q = c; int *r = d; }
template <class I> struct t1 { typedef I v __attribute__((vector_size(4 * sizeof(I)))); typedef I w; };
template <class I> using A = typename t1<I>::v;
template <class I> using A2 = typename t1<I>::w;
template <class I> void k() { A<I> x; A2<I> y; int *p = x; int *q = y; }
template void k<long>();
template <class I> struct t2 { using E = A<I>; using F = A2<I>; void g(E e, F f) { int *p = e; int *q = f; } };
template struct t2<short>;
