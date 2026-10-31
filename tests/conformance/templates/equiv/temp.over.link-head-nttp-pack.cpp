//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:16 error "template non-type parameter has a different type 'long' in template redeclaration"
//@ diag: 9:15 note "previous non-type template parameter with type 'int' is here"
//@ diag: 12:17 error "template type parameter conflicts with previous template type parameter pack"
//@ diag: 11:20 note "previous template type parameter pack declared here"
// [temp.over.link]/6: non-type parameters of different types, and a pack
// against a non-pack.
template <int N> struct D;
template <long N> struct D;
template <class... T> struct E;
template <class T> struct E;
