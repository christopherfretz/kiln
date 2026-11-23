// [temp.variadic]/11, [temp.dep.candidate]/1, [over.match.oper]/3.4: the
// operator expressions a fold-expression instantiates to take the
// non-member candidates its definition found; an operator declared after
// the definition, in no associated namespace of the operands, is not a
// candidate, and clang names it ("neither visible in the template
// definition ..."), for a right and a left unary fold and a binary fold
// alike; a rewritten `!=` from a later `==` is not found either.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:51 error "call to function 'operator+' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 31:9 note "in instantiation of function template specialization 'r<N::S, N::S>' requested here"
//@ diag: 27:6 note "'operator+' should be declared prior to the call site or in namespace 'N'"
//@ diag: 23:47 error "call to function 'operator-' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 31:31 note "in instantiation of function template specialization 'l<N::S, N::S>' requested here"
//@ diag: 28:6 note "'operator-' should be declared prior to the call site or in namespace 'N'"
//@ diag: 24:59 error "call to function 'operator*' that is neither visible in the template definition nor found by argument-dependent lookup"
//@ diag: 31:53 note "in instantiation of function template specialization 'b<int>' requested here"
//@ diag: 29:6 note "'operator*' should be declared prior to the call site or in namespace 'N'"
//@ diag: 25:53 error "invalid operands to binary expression ('N::S' and 'N::S')"
//@ diag: 31:69 note "in instantiation of function template specialization 'e<N::S, N::S>' requested here"
namespace N { struct S { int v; }; }
template <class... T> int r(T... t) { return (t + ...).v; }
template <class... T> int l(T... t) { return (... - t).v; }
template <class... T> int b(N::S s, T... t) { return (s * ... * t).v; }
template <class... T> bool e(T... t) { return (t != ...); }
// Declared after the definitions, in no associated namespace of `N::S`.
N::S operator+(N::S a, N::S) { return a; }
N::S operator-(N::S a, N::S) { return a; }
N::S operator*(N::S a, int) { return a; }
bool operator==(N::S, N::S) { return true; }
int x = r(N::S{1}, N::S{2}) + l(N::S{1}, N::S{2}) + b(N::S{1}, 2) + e(N::S{1}, N::S{2});
// One element: no operator is applied.
int y = r(N::S{1});
