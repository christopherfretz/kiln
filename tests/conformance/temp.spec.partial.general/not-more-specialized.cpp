// [temp.spec.partial.general]/9.2: a partial specialization shall be more
// specialized than the primary template (clang checkMoreSpecializedThanPrimary),
// for class and variable templates; a genuinely more specialized partial
// (A<T*, U>, B<N, N>, v<T, int>, E<T, N>) stays valid. Found by the clang
// negative-test run (WP-912), fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:42 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 15:42 note "template is declared here"
//@ diag: 19:32 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 18:32 note "template is declared here"
//@ diag: 22:39 error "variable template partial specialization is not more specialized than the primary template"
//@ diag: 21:39 note "template is declared here"
template <typename T, typename U> struct A {};
template <typename T, typename U> struct A<U, T> {};
template <typename T, typename U> struct A<T *, U> {};
template <int N, int M> struct B {};
template <int N, int M> struct B<M, N> {};
template <int N> struct B<N, N> {};
template <typename T, typename U> int v = 0;
template <typename T, typename U> int v<U, T> = 1;
template <typename T> int v<T, int> = 2;
template <class T, int N> struct E {};
template <class T, T N> struct E<T, N> {};
A<int *, char> a;
B<1, 1> b;
int i = v<char, int>;
E<int, 3> e;
