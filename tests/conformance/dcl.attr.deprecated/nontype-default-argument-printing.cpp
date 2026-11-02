// [dcl.attr.deprecated]/4, [temp.arg.nontype] (WP-297): a deprecated class
// template specialization is named with its arguments, dropping a trailing
// non-type argument only when its default is a non-dependent integral
// constant expression of the same value (clang's
// isSubstitutedTemplateArgument): `Lit<1, 2>` prints as `Lit<1>`, but a
// default naming an earlier parameter or a dependent expression keeps the
// argument (libc++'s `aligned_storage<1, 1>`).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 30:1 warning "'Store<1, 1>' is deprecated"
//@ diag: 25:50 note "'Store<1, 1>' has been explicitly marked deprecated here"
//@ diag: 31:1 warning "'Store<8>' is deprecated"
//@ diag: 25:50 note "'Store<8>' has been explicitly marked deprecated here"
//@ diag: 32:1 warning "'Pair<3, 3>' is deprecated"
//@ diag: 26:38 note "'Pair<3, 3>' has been explicitly marked deprecated here"
//@ diag: 33:1 warning "'Pair<3, 4>' is deprecated"
//@ diag: 26:38 note "'Pair<3, 4>' has been explicitly marked deprecated here"
//@ diag: 34:1 warning "'Paren<5, 5>' is deprecated"
//@ diag: 27:40 note "'Paren<5, 5>' has been explicitly marked deprecated here"
//@ diag: 35:1 warning "'Lit<1>' is deprecated"
//@ diag: 28:38 note "'Lit<1>' has been explicitly marked deprecated here"
//@ diag: 36:1 warning "'Lit<1, 3>' is deprecated"
//@ diag: 28:38 note "'Lit<1, 3>' has been explicitly marked deprecated here"
template <int L> struct Max { static constexpr int value = L > 4 ? 4 : L; };
template <int L, int A = Max<L>::value> struct [[deprecated]] Store {};
template <int N, int M = N> struct [[deprecated]] Pair {};
template <int N, int M = (N)> struct [[deprecated]] Paren {};
template <int N, int M = 2> struct [[deprecated]] Lit {};

Store<1, 1> s1;
Store<8> s2;
Pair<3, 3> p1;
Pair<3, 4> p2;
Paren<5, 5> q1;
Lit<1, 2> l1;
Lit<1, 3> l2;
