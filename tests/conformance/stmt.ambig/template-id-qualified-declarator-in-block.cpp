// [stmt.ambig]/1, [dcl.meaning.general]: `Hog<1>(Hog<1>::OINK);` in a
// block is a declaration (of a variable `Hog<1>::OINK`), not a functional
// cast, so it is diagnosed as a qualified declarator in a function, as
// `P(P::Q);` is. kiln parses the template-id form as an expression and
// accepts it (reduced from GCC's g++.dg/cpp2a/using-enum-10a.C).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ xfail(kiln): WP-1734 follow-up, a template-id functional cast whose operand is a qualified-id is not disambiguated as a declaration
//@ diag: 16:27 error "definition or redeclaration of 'OINK' not allowed inside a function"
//@ diag: 18:17 error "definition or redeclaration of 'Q' not allowed inside a function"

template <int N> struct Hog { enum E { OINK }; Hog(E) {} };
struct P { enum E { Q }; P(E) {} };

void f() { Hog<1>(Hog<1>::OINK); }

void g() { P(P::Q); }
