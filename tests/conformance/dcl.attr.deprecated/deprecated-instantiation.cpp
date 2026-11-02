// [dcl.attr.deprecated]/4, [temp.inst] (WP-297): a deprecated class template
// named in a template is warned when the template is defined if the name is
// not dependent, and again with instantiation notes when an instantiation
// forms the specialization.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 36:38 warning "'S' is deprecated"
//@ diag: 35:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 37:19 warning "'P<long>' is deprecated"
//@ diag: 39:32 note "in instantiation of template type alias 'A' requested here"
//@ diag: 34:28 note "'P<long>' has been explicitly marked deprecated here"
//@ diag: 39:43 warning "'P<short>' is deprecated"
//@ diag: 34:28 note "'P<short>' has been explicitly marked deprecated here"
//@ diag: 39:57 warning "'S' is deprecated"
//@ diag: 35:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 36:30 warning "'P<char>' is deprecated"
//@ diag: 40:15 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 34:28 note "'P<char>' has been explicitly marked deprecated here"
//@ diag: 41:12 warning "'P<int>' is deprecated"
//@ diag: 34:28 note "'P<int>' has been explicitly marked deprecated here"
//@ diag: 41:22 warning "'S' is deprecated"
//@ diag: 35:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 42:8 warning "'P<int>' is deprecated"
//@ diag: 34:28 note "'P<int>' has been explicitly marked deprecated here"
//@ diag: 43:1 warning "'P<char>' is deprecated"
//@ diag: 34:28 note "'P<char>' has been explicitly marked deprecated here"
//@ diag: 36:30 warning "'P<int>' is deprecated"
//@ diag: 39:12 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 34:28 note "'P<int>' has been explicitly marked deprecated here"
//@ diag: 37:19 warning "'P<int>' is deprecated"
//@ diag: 38:30 note "in instantiation of template type alias 'A' requested here"
//@ diag: 39:22 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 34:28 note "'P<int>' has been explicitly marked deprecated here"
template<class T> struct [[deprecated]] P { P() {} };
struct [[deprecated]] S {};
template<class T> void f() { P<T> x; S s; (void)x; }
template<class T> using A = P<T>;
template<class T> void g() { A<T> y; }
void h() { f<int>(); g<int>(); A<long> z; P<short> *pp; S *ps; }
template void f<char>();
struct X { P<int> m; S n; };
void k(P<int>);
P<char> r();
