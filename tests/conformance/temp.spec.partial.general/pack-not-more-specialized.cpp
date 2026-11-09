// [temp.spec.partial.general]/9.2 with packs: a partial specialization whose
// pack expansion takes the place of non-pack primary parameters is not more
// specialized than the primary (clang isMoreSpecializedThanPrimary checks
// both directions; DR1495). The trailing valid partials (R, D) stay usable.
// Found by the clang negative-test run (WP-912), fixed by WP-956.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:34 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 19:41 note "template is declared here"
//@ diag: 22:34 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 21:46 note "template is declared here"
//@ diag: 24:31 error "variable template partial specialization is not more specialized than the primary template"
//@ diag: 23:38 note "template is declared here"
//@ diag: 26:31 error "variable template partial specialization is not more specialized than the primary template"
//@ diag: 25:43 note "template is declared here"
//@ diag: 28:34 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 27:58 note "template is declared here"
template <typename, typename...> struct B {};
template <typename... Ts> struct B<Ts...> {};
template <int, typename, typename...> struct C {};
template <typename... Ts> struct C<0, Ts...> {};
template <typename, typename...> int b = 0;
template <typename... Ts> int b<Ts...> = 1;
template <int, typename, typename...> int c = 0;
template <typename... Ts> int c<0, Ts...> = 1;
template <typename T, typename U, typename V = U> struct X0;
template <typename... Ts> struct X0<Ts...> {};
template <class... T> struct R {};
template <class T, class... U> struct R<T, U...> {};
template <class T, class... U> struct D {};
template <class T, class... U> struct D<T *, U...> {};
R<int, char> r;
D<int *, char> d;
