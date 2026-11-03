// [temp.arg.general]/1: each template-argument is checked against its
// template-parameter in order; clang's `CheckTemplateArgumentList` stops at the
// first that fails, so later arguments and the arity are not reported.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:3 error "template argument for template type parameter must be a type"
//@ diag: 20:17 note "template parameter is declared here"
//@ diag: 26:3 error "use of class template 'K' requires template arguments"
//@ diag: 19:25 note "template is declared here"
//@ diag: 27:11 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 28:3 error "template argument for template type parameter must be a type"
//@ diag: 21:17 note "template parameter is declared here"
//@ diag: 29:8 error "template argument for non-type template parameter must be an expression"
//@ diag: 21:24 note "template parameter is declared here"
//@ diag: 30:8 error "template argument for template type parameter must be a type"
//@ diag: 22:20 note "template parameter is declared here"
//@ diag: 32:12 error "no matching function for call to 'ff'"
//@ diag: 31:32 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'T'"
template <class> struct K {};
template <class T, int N, template <class> class TT> struct M {};
template <class T, int N> struct Q {};
template <class... Ts> struct P {};

// Only the first failing argument is reported.
M<1, int, 2> m1;
M<K, K, K> m2;
M<int, 1, int> m3;
Q<1, int> q1;
Q<int, int, int> q2;
P<int, 1, 2> p1;
template <class T, int N> void ff();
void g() { ff<1, int>(); }
