// [temp.spec.partial.general]/9.3 with packs: a partial specialization whose
// arguments are exactly the primary's parameter packs, in order, does not
// specialize any template argument. Found by the clang negative-test run
// (WP-912), fixed by WP-956.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:30 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 19:30 error "redefinition of 'P'"
//@ diag: 18:30 note "previous definition is here"
//@ diag: 21:28 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 21:28 error "redefinition of 'Q'"
//@ diag: 20:28 note "previous definition is here"
//@ diag: 23:48 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 23:48 error "redefinition of 'S'"
//@ diag: 22:48 note "previous definition is here"
//@ diag: 25:27 error "variable template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
template <class... T> struct P {};
template <class... T> struct P<T...> {};
template <int... N> struct Q {};
template <int... N> struct Q<N...> {};
template <template <class> class... TT> struct S {};
template <template <class> class... TT> struct S<TT...> {};
template <class... T> int v = 0;
template <class... T> int v<T...> = 1;
