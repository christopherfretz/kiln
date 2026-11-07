// [temp.spec.partial.general]/9.3, [temp.over.link]/6: a partial
// specialization whose non-type parameters have the primary template's
// types, in order, is the primary template's own argument list; one
// whose parameter is more specific than the primary's `auto` is not
// (WP-789).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:32 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 14:26 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ std: c++20
template <class T, T V> struct X;
template <class T, T V> struct X<T, V> {};
template <auto V> struct Y;
template <auto V> struct Y<V> {};
template <auto V> struct Z;
template <int V> struct Z<V> {};
template <class T, auto V> struct W;
template <class T, T V> struct W<T, V> {};
