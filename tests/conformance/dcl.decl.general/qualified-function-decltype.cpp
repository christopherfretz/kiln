// [dcl.fct]/6, [dcl.type.decltype]: a qualified function type reached
// through a member typedef of a class template specialization or through
// `decltype` of a pointer to member function (`int () const` taken apart
// by a partial specialization) is diagnosed as one named by a plain
// typedef-name: as a non-member function, a static member function or an
// `extern` declaration it is an error; as a non-static member it declares
// a member function. Matches clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:1 error "non-member function of type 'X<F>::type' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 21:1 error "non-member function of type 'mp<decltype(&M::f)>::type' (aka 'int () const') cannot have 'const' qualifier"
//@ diag: 23:1 error "non-member function of type 'H' (aka 'int () &&') cannot have '&&' qualifier"
//@ diag: 24:18 error "static member function of type 'H' (aka 'int () &&') cannot have '&&' qualifier"
//@ diag: 25:1 error "non-member function of type 'H' (aka 'int () &&') cannot have '&&' qualifier"
typedef int F() const;
template <class T> struct X { using type = T; };
X<F>::type xt;
struct M { int f() const; int g() &&; };
template <class> struct mp;
template <class C, class P> struct mp<P C::*> { using type = P; };
mp<decltype(&M::f)>::type a;
using H = mp<decltype(&M::g)>::type;
H b;
struct N { H ok; static H bad; };
extern H e;
