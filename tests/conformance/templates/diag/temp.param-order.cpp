//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:32 error "template parameter missing a default argument"
//@ diag: 11:21 note "previous default template argument defined here"
//@ diag: 12:20 error "template parameter pack must be the last template parameter"
//@ diag: 13:18 error "template parameter pack must be the last template parameter"
//@ diag: 14:28 error "template parameter missing a default argument"
//@ diag: 14:19 note "previous default template argument defined here"
// [temp.param]/14: in a class, variable or alias template, parameters after
// a default have defaults and a pack comes last; function templates are exempt.
template <class T = int, class U> struct A;
template <class... Ts, class U> struct P;
template <int... Ns, int M> constexpr int v = 0;
template <int N = 1, class T> using Al = T;
template <class T = int, class U> void f(U);
template <class... Ts, class U> void g(U);
template <class T = int, class... Ts> struct Q;
