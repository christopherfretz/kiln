// [temp.res.general]: errors found after substitution point at the name as
// written (spec/sema-templates.md §5.8), or at the 'using' of an alias template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:43 error "no type named 'nope' in 'P'"
//@ diag: 26:7 note "in instantiation of template class 'W1<P>' requested here"
//@ diag: 27:43 error "typename specifier refers to non-type member 'notype' in 'P'"
//@ diag: 28:7 note "in instantiation of template class 'W2<P>' requested here"
//@ diag: 24:16 note "referenced member 'notype' is declared here"
//@ diag: 29:40 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 30:9 note "in instantiation of template class 'W3<int>' requested here"
//@ diag: 31:43 error "no member named 'N' in 'P'"
//@ diag: 32:7 note "in instantiation of template class 'W4<P>' requested here"
//@ diag: 33:46 error "no type named 'R' in 'P::Q'"
//@ diag: 34:7 note "in instantiation of template class 'W5<P>' requested here"
//@ diag: 35:52 error "no member named 'Z' in 'P'"
//@ diag: 36:7 note "in instantiation of template class 'W6<P>' requested here"
//@ diag: 37:19 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 38:1 note "in instantiation of template type alias 'X1' requested here"
//@ diag: 37:19 error "no type named 'nope' in 'P'"
//@ diag: 39:1 note "in instantiation of template type alias 'X1' requested here"
//@ diag: 40:19 error "cannot form a reference to 'void'"
//@ diag: 41:1 note "in instantiation of template type alias 'X2' requested here"
struct P { int notype; struct Q {}; };
template<class T> struct W1 { typename T::nope v; };
W1<P> w1;
template<class T> struct W2 { typename T::notype v; };
W2<P> w2;
template<class T> struct W3 { typename T::type v; };
W3<int> w3;
template<class T> struct W4 { typename T::N::V v; };
W4<P> w4;
template<class T> struct W5 { typename T::Q::R v; };
W5<P> w5;
template<class T> struct W6 { typename T::template Z<int> v; };
W6<P> w6;
template<class T> using X1 = typename T::nope;
X1<int> x1;
X1<P> x2;
template<class T> using X2 = T&;
X2<void> x3;
