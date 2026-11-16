// [temp.dep.candidate]/1, [over.match.oper]/3: an operator or function
// declared after a template's definition in a namespace that is not
// associated with the arguments is not a candidate at instantiation; clang
// names it ("neither visible in the template definition nor found by
// argument-dependent lookup") and suggests the one associated namespace,
// any associated namespace, or none (WP-1510).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:41 error "call to function 'operator-' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 36:9 note "in instantiation of function template specialization 'h<N::S>' requested here"
//@ diag: 31:5 note "'operator-' should be declared prior to the call site or in namespace 'N'"
//@ diag: 27:39 error "call to function 'operator~' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 36:21 note "in instantiation of function template specialization 'c<N::S>' requested here"
//@ diag: 32:5 note "'operator~' should be declared prior to the call site or in namespace 'N'"
//@ diag: 28:39 error "call to function 'f' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 36:33 note "in instantiation of function template specialization 'k<N::S>' requested here"
//@ diag: 33:5 note "'f' should be declared prior to the call site or in namespace 'N'"
//@ diag: 29:54 error "call to function 'g' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 36:45 note "in instantiation of function template specialization 'k2<N::S, M::T>' requested here"
//@ diag: 34:5 note "'g' should be declared prior to the call site or in an associated namespace of one of its arguments"
//@ diag: 30:40 error "call to function 'q' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 36:66 note "in instantiation of function template specialization 'k3<int>' requested here"
//@ diag: 35:5 note "'q' should be declared prior to the call site"
namespace N { struct S {}; }
namespace M { struct T {}; }
template<class X> int h(X x) { return x - 1; }
template<class X> int c(X x) { return ~x; }
template<class X> int k(X x) { return f(x); }
template<class X, class Y> int k2(X x, Y y) { return g(x, y); }
template<class X> int k3(X x) { return q(x); }
int operator-(N::S, int) { return 2; }
int operator~(N::S) { return 3; }
int f(N::S) { return 4; }
int g(N::S, M::T) { return 5; }
int q(int) { return 6; }
int b = h(N::S{}) + c(N::S{}) + k(N::S{}) + k2(N::S{}, M::T{}) + k3(1);
// A later operator that cannot take the operands is not named: the
// built-in unary `&` applies.
template<class X> int a(const X& x) { return sizeof(&x); }
enum class E : unsigned char {};
E operator&(E, E);
int d = a(N::S{});
