// [temp.spec.partial.general]/9.2, [temp.deduct.type]/20: a partial
// specialization must be more specialized than its primary even when a
// non-type parameter's type is a placeholder (`auto N`, `decltype(auto) N`)
// or another parameter (`T N`); that type is deduced from the synthesized
// argument, and a synthesized `auto` is a unique type that only a
// placeholder or a type parameter accepts (clang `isMoreSpecializedThanPrimary`
// with `DeduceTemplateArguments` on the parameter type). Before WP-1105 kiln
// skipped the check for such a primary parameter and accepted all of these.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:34 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 24:32 note "template is declared here"
//@ diag: 27:32 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 26:26 note "template is declared here"
//@ diag: 29:26 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 28:25 note "template is declared here"
//@ diag: 31:40 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 30:25 note "template is declared here"
//@ diag: 33:36 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 32:25 note "template is declared here"
//@ diag: 35:32 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 34:25 note "template is declared here"
template <class T, T N> struct B;
template <class T, int N> struct B<T, N> {};
template <auto N> struct A;
template <class T, T N> struct A<N> {};
template <int N> struct F;
template <auto N> struct F<N> {};
template <int N> struct Z;
template <auto N> requires true struct Z<N> {};
template <int N> struct V2;
template <decltype(auto) N> struct V2<N> {};
template <int N> struct X;
template <class T, T V> struct X<V> {};
// Valid: the partial's type is more specific than the primary's
// placeholder, or a non-type parameter of another fixed type.
template <auto N> struct OK1;
template <int N> struct OK1<N> {};
template <auto N> struct OK2;
template <class T, T *P> struct OK2<P> {};
template <int N> struct OK3;
template <short S> struct OK3<S> {};
template <auto N> struct OK4;
template <auto N> requires true struct OK4<N> {};
