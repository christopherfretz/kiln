// [temp.arg.general]/2: the number of template arguments must match the
// parameters (after defaults); the note names the template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:1 error "too many template arguments for class template 'One'"
//@ diag: 13:27 note "template is declared here"
//@ diag: 16:1 error "too few template arguments for class template 'Two'"
//@ diag: 15:36 note "template is declared here"
//@ diag: 18:1 error "too few template arguments for class template 'S'"
//@ diag: 17:27 note "template is declared here"
//@ diag: 20:1 error "too few template arguments for class template 'P'"
//@ diag: 19:42 note "template is declared here"
template <class T> struct One {};
One<int, int> a;
template <class T, class U> struct Two {};
Two<int> b;
template <class T> struct S {};
S<> s;
template <class T, class U = int> struct P {};
P<> p;
