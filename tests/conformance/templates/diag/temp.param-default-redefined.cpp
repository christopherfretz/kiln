//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:21 error "template parameter redefines default argument"
//@ diag: 11:21 note "previous default template argument defined here"
//@ diag: 14:19 error "template parameter redefines default argument"
//@ diag: 13:19 note "previous default template argument defined here"
//@ diag: 16:36 error "template parameter redefines default argument"
//@ diag: 15:30 note "previous default template argument defined here"
// [temp.param]/12: a default template argument is not specified twice;
// one given by an earlier declaration is inherited.
template <class T = int> struct B;
template <class T = int> struct B {};
template <int N = 1> void g();
template <int N = 2> void g();
template <class T, class U = int> struct X;
template <class T = int, class U = long> struct X;
template <class T, class U = int> struct Y;
template <class T = int, class U> struct Y;
