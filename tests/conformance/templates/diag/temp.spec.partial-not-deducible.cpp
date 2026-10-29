// [temp.spec.partial.general]/8 (clang -Wunusable-partial-specialization):
// a partial specialization whose template parameters cannot all be deduced
// from its arguments can never be used.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:25 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 13:15 note "non-deducible template parameter 'N'"
//@ diag: 15:36 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 15:26 note "non-deducible template parameter 'U'"
//@ diag: 17:43 error "variable template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 17:26 note "non-deducible template parameter 'U'"
template <int N, bool B> struct parity {};
template <int N> struct parity<0, N == 0> {};
template <class T, class U> struct pair {};
template <class T, class U> struct pair<T, int> {};
template <class T, class U> constexpr int v = 0;
template <class T, class U> constexpr int v<T *, int> = 1;
