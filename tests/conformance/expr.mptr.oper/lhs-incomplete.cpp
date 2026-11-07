// [expr.mptr.oper]/2-3: the left operand of `.*` whose class differs from
// the member pointer's must be complete to be checked for derivation; an
// incomplete class is an error with its forward declaration noted, an
// undefined class template specialization only the instantiation error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:40 error "left hand operand to .* must be a class compatible with the right hand operand, but is 'I'"
//@ diag: 11:8 note "forward declaration of 'I'"
//@ diag: 16:45 error "implicit instantiation of undefined template 'U<int>'"
//@ diag: 14:27 note "template is declared here"
struct I;
template <class T> struct B { int n; };
template <class T> struct D : B<T> {};
template <class T> struct U;
int f(I& i, int B<int>::* p) { return i.*p; }
int g(U<int>& u, int B<int>::* p) { return u.*p; }
int h(D<int>* d, int B<int>::* p) { return d->*p; }
