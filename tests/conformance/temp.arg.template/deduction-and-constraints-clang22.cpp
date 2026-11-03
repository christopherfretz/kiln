// [temp.arg.template]/3 (P0522, clang 22 matching): P's parameters are
// deduced against A's; a conflicting deduction of A's type parameter and a
// constrained P that does not subsume A's constraints ([temp.constr.order])
// reject the argument; in explicit function template arguments the error is
// a SFINAE failure (spec/sema-templates.md §3.2.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:12 error "no matching function for call to 'h2'"
//@ diag: 20:52 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'TT'"
//@ diag: 26:21 error "no matching function for call to 'h6'"
//@ diag: 24:42 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'TT'"
//@ diag: 19:17 error "conflicting deduction 'U' against 'int' for parameter"
//@ diag: 27:4 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 21:43 note "previous template template parameter is here"
//@ diag: 28:4 error "template template argument 'CS' is more constrained than template template parameter 'TT'"
//@ diag: 25:33 note "'TT' declared here"
//@ diag: 23:23 note "'CS' declared here"
//@ requires: clang>=22
template <class U, U V> struct X {};
template <template <class U, int V> class TT> void h2() {}
template <template <class U, int V> class TT> struct H2 {};
template <class T> concept C = sizeof(T) >= 1;
template <C T> struct CS {};
template <template <C...> class TT> void h6() {}
template <template <C...> class TT> struct H6 {};
void u() { h2<X>(); h6<CS>(); }
H2<X> a;
H6<CS> b;
